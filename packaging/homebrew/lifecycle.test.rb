# Fixture test of the embedded Cask hooks, without Homebrew or OS modifications.
require "pathname"
require "tmpdir"
require "json"
require "fileutils"

class Pathname
  # Homebrew provides atomic_write. Model that boundary without importing Homebrew.
  def atomic_write(content)
    write(content)
  end
end

class CaskFixture
  attr_reader :loaded, :hooks
  attr_accessor :fail_bootstrap

  def initialize
    @loaded = {}
    @hooks = {}
  end

  def postflight(&block)
    @hooks[:install] = block
  end

  def uninstall_preflight(&block)
    @hooks[:uninstall] = block
  end

  def run(hook)
    instance_eval(&@hooks.fetch(hook))
  end

  def system_command(command, args:, must_succeed: true)
    raise "Unexpected command" unless command == "/bin/launchctl"

    case args[0]
    when "print"
      Struct.new(:success?).new(@loaded.key?(args[1]))
    when "bootstrap"
      raise "Bootstrap fixture failure" if fail_bootstrap

      name = File.basename(args[2], ".plist")
      @loaded["#{args[1]}/#{name}"] = File.read(args[2])
      Struct.new(:success?).new(true)
    else
      raise "Unexpected launchctl operation"
    end
  end
end

def check(condition, message)
  raise message unless condition
end

original_home = Dir.method(:home)
Dir.mktmpdir("p01-cask-") do |home|
  Dir.define_singleton_method(:home) { home }
  begin
    fixture = CaskFixture.new
    fixture.instance_eval(File.read(File.join(__dir__, "lifecycle.rb")))
    agents = Pathname(home)/"Library/LaunchAgents"
    agents.mkpath
    active = agents/"de.openspider.task.active.plist"
    inactive = agents/"de.openspider.task.inactive.plist"
    unrelated = agents/"org.other.task.plist"
    active.write("active fixture")
    inactive.write("inactive fixture")
    unrelated.write("other fixture")
    domain = "gui/#{Process.uid}"
    fixture.loaded["#{domain}/de.openspider.task.active"] = active.read
    snapshot = Pathname(home)/"Library/Application Support/OpenSpider/homebrew-launchagents.json"

    # spec: OUT-166 [M3] — DSL fixture of loaded-only preservation and uninstall removal.
    fixture.run(:uninstall)
    entries = JSON.parse(snapshot.read)
    check(entries.size == 1 && entries[0]["name"] == active.basename.to_s, "Inactive task was backed up")
    check((snapshot.stat.mode & 0o777) == 0o600, "Snapshot permissions")
    active.unlink
    inactive.unlink
    fixture.loaded.clear
    check(!active.exist? && fixture.loaded.empty?, "Uninstall left registrations")
    fixture.run(:install)
    check(active.read == "active fixture", "Active plist lost on upgrade")
    check(fixture.loaded.key?("#{domain}/de.openspider.task.active"), "Active registration not restored")
    check(!inactive.exist? && unrelated.read == "other fixture", "Inactive/unrelated job changed")
    check(!snapshot.exist?, "Snapshot not consumed")
    puts "ok: active task survives upgrade; inactive stays unregistered; unrelated task unchanged"

    fixture.run(:uninstall)
    active.unlink
    fixture.loaded.clear
    fixture.fail_bootstrap = true
    begin
      fixture.run(:install)
      raise "Expected bootstrap failure"
    rescue RuntimeError => error
      raise unless error.message == "Bootstrap fixture failure"
    end
    check(snapshot.exist? && !active.exist?, "Failed restoration not retryable")
    fixture.fail_bootstrap = false
    fixture.run(:install)
    check(active.exist? && !snapshot.exist?, "Restore retry failed")
    puts "ok: failed bootstrap retains snapshot and can be retried"

    fixture.run(:uninstall)
    active.write("independent registration")
    fixture.run(:install)
    check(active.read == "independent registration", "Independent registration overwritten")
    puts "ok: independently created registration preserved"
  ensure
    Dir.define_singleton_method(:home, original_home)
  end
end
