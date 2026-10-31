  # Homebrew runs uninstall directives during upgrade too. Preserve only loaded jobs;
  # a later install restores the last active registrations, never inactive tasks.
  postflight do
    require "json"
    snapshot = Pathname(Dir.home)/"Library/Application Support/OpenSpider/homebrew-launchagents.json"
    if snapshot.exist?
      raise "LaunchAgent snapshot is a symlink" if snapshot.symlink?

      JSON.parse(snapshot.read).each do |entry|
        name = entry.fetch("name")
        unless File.basename(name) == name && name.start_with?("de.openspider.task.") && name.end_with?(".plist")
          raise "Invalid OpenSpider LaunchAgent name"
        end
        domain = entry.fetch("domain")
        raise "Invalid LaunchAgent domain" unless ["gui/#{Process.uid}", "user/#{Process.uid}"].include?(domain)

        target = Pathname(Dir.home)/"Library/LaunchAgents"/name
        target.dirname.mkpath
        raise "LaunchAgent target is a symlink" if target.symlink?

        # Never overwrite a registration created independently since uninstall.
        unless target.exist?
          target.atomic_write(entry.fetch("plist"))
          begin
            system_command "/bin/launchctl", args: ["bootstrap", domain, target.to_s], must_succeed: true
          rescue StandardError
            target.unlink
            raise
          end
        end
      end
      snapshot.unlink
    end
  end

  uninstall_preflight do
    require "json"
    snapshot = Pathname(Dir.home)/"Library/Application Support/OpenSpider/homebrew-launchagents.json"
    snapshot.dirname.mkpath
    raise "LaunchAgent snapshot is a symlink" if snapshot.symlink?

    entries = []
    Dir.glob("#{Dir.home}/Library/LaunchAgents/de.openspider.task.*.plist").sort.each do |path|
      plist = Pathname(path)
      raise "LaunchAgent is a symlink" if plist.symlink?

      label = plist.basename(".plist").to_s
      ["gui/#{Process.uid}", "user/#{Process.uid}"].each do |domain|
        loaded = system_command "/bin/launchctl", args: ["print", "#{domain}/#{label}"], must_succeed: false
        next unless loaded.success?

        entries << { "name" => plist.basename.to_s, "domain" => domain, "plist" => plist.read }
        break
      end
    end
    snapshot.atomic_write(JSON.generate(entries))
    snapshot.chmod(0o600)
  end
