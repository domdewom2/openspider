# Fixture of the generated script. No Homebrew or OS registrations are modified.
require_relative "lifecycle"
require "tmpdir"

def check(condition, message)
  raise message unless condition
end

Dir.mktmpdir("p01-cask-") do |home|
  loaded = {}
  fail_bootstrap = false
  launchctl = lambda do |args, must_succeed:|
    case args[0]
    when "print"
      loaded.key?(args[1])
    when "bootout"
      check(loaded.key?(args[1]), "Unexpected bootout target")
      loaded.delete(args[1])
      true
    when "bootstrap"
      raise "Bootstrap fixture failure" if fail_bootstrap

      name = File.basename(args[2], ".plist")
      loaded["#{args[1]}/#{name}"] = File.read(args[2])
      true
    else
      raise "Unexpected launchctl operation"
    end
  end
  lifecycle = OpenSpiderLaunchAgents.new(home: home, launchctl: launchctl)
  agents = File.join(home, "Library/LaunchAgents")
  FileUtils.mkdir_p(agents)
  active = File.join(agents, "de.openspider.task.active.plist")
  inactive = File.join(agents, "de.openspider.task.inactive.plist")
  unrelated = File.join(agents, "org.other.task.plist")
  File.write(active, "active fixture")
  File.write(inactive, "inactive fixture")
  File.write(unrelated, "other fixture")
  domain = "gui/#{Process.uid}"
  user_domain = "user/#{Process.uid}"
  loaded["#{domain}/de.openspider.task.active"] = File.read(active)
  loaded["#{user_domain}/de.openspider.task.active"] = File.read(active)
  snapshot = File.join(home, "operation-temp/launchagents.json")

  # // spec: OUT-166 [M3]
  lifecycle.save(snapshot)
  entries = JSON.parse(File.read(snapshot))
  check(entries.size == 2 && entries.sum { |e| e["domains"].size } == 2, "Activation state lost")
  check(loaded.empty?, "Uninstall left a registration in the other domain")
  check((File.stat(snapshot).mode & 0o777) == 0o600, "Snapshot permissions")
  File.unlink(active)
  File.unlink(inactive)
  loaded.clear # Homebrew's uninstall directives unload and delete these jobs.
  lifecycle.restore(snapshot)
  check(File.read(active) == "active fixture", "Active plist lost on upgrade")
  check(loaded.key?("#{domain}/de.openspider.task.active"), "Active registration not restored")
  check(loaded.key?("#{user_domain}/de.openspider.task.active"), "Other domain not restored")
  check(File.read(inactive) == "inactive fixture", "Inactive definition lost")
  check(!loaded.key?("#{domain}/de.openspider.task.inactive"), "Inactive task activated")
  check(File.read(unrelated) == "other fixture", "Unrelated job changed")
  check(!File.exist?(snapshot), "Snapshot not consumed")
  puts "ok: upgrade preserves active and inactive definitions and activation state"

  lifecycle.save(snapshot)
  File.unlink(active)
  loaded.clear
  fail_bootstrap = true
  begin
    lifecycle.restore(snapshot)
    raise "Expected bootstrap failure"
  rescue RuntimeError => error
    raise unless error.message == "Bootstrap fixture failure"
  end
  check(File.exist?(snapshot) && !File.exist?(active), "Failed restoration not retryable")
  fail_bootstrap = false
  lifecycle.restore(snapshot)
  check(File.exist?(active) && !File.exist?(snapshot), "Restore retry failed")
  puts "ok: failed bootstrap retains snapshot and can be retried"

  lifecycle.save(snapshot)
  File.write(active, "independent registration")
  lifecycle.restore(snapshot)
  check(File.read(active) == "independent registration", "Independent registration overwritten")
  # Separate install after normal uninstall gets a new operation temp, so no resurrection.
  loaded.clear
  File.unlink(active)
  File.unlink(inactive)
  lifecycle.restore(File.join(home, "next-operation-temp/launchagents.json"))
  check(loaded.empty? && !File.exist?(active), "Separate reinstall resurrected registrations")
  puts "ok: independent registrations preserved; separate reinstall does not restore uninstalled jobs"
end
