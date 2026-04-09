cask "ai-toolbox" do
  version "0.7.9"

  on_arm do
    sha256 "df1137c5e01ed54255b8068daabe6158ddcb89debcb62e9bdbb4d44476a880ae"
    url "https://github.com/coulsontl/ai-toolbox/releases/download/v#{version}/AI.Toolbox_0.7.9_aarch64.dmg",
        verified: "github.com/coulsontl/ai-toolbox/"
  end

  on_intel do
    sha256 "96d5eb6426b195477726921438cc0c70f2304f1b6e1afd516a6dab6a60ca14bd"
    url "https://github.com/coulsontl/ai-toolbox/releases/download/v#{version}/AI.Toolbox_0.7.9_x64.dmg",
        verified: "github.com/coulsontl/ai-toolbox/"
  end

  name "AI Toolbox"
  desc "Desktop toolbox for managing AI coding assistant configurations"
  homepage "https://github.com/coulsontl/ai-toolbox"

  app "AI Toolbox.app"
end
