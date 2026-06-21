cask "ai-toolbox" do
  version "0.9.9"

  on_arm do
    sha256 "7fe4265974f7b3a564e6f0d578bec87b33f31174f47ba8c528500640135720df"
    url "https://github.com/coulsontl/ai-toolbox/releases/download/v#{version}/AI.Toolbox_0.9.9_aarch64.dmg",
        verified: "github.com/coulsontl/ai-toolbox/"
  end

  on_intel do
    sha256 "76574c4ccc51f5ae10c52f2c7ac546ea3911fe66cfe293b0b29cb70a47d78656"
    url "https://github.com/coulsontl/ai-toolbox/releases/download/v#{version}/AI.Toolbox_0.9.9_x64.dmg",
        verified: "github.com/coulsontl/ai-toolbox/"
  end

  name "AI Toolbox"
  desc "Desktop toolbox for managing AI coding assistant configurations"
  homepage "https://github.com/coulsontl/ai-toolbox"

  app "AI Toolbox.app"
end
