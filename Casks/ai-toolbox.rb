cask "ai-toolbox" do
  version "0.8.5"

  on_arm do
    sha256 "d10cd43ace02c7c5e466550e24337d737ddcd6f28de3a99e5a53c94a319a5065"
    url "https://github.com/coulsontl/ai-toolbox/releases/download/v#{version}/AI.Toolbox_0.8.5_aarch64.dmg",
        verified: "github.com/coulsontl/ai-toolbox/"
  end

  on_intel do
    sha256 "25a7c34a690ee745c5f82eb417ae63ed8ae306274d32907fe9b5bd66561d45c6"
    url "https://github.com/coulsontl/ai-toolbox/releases/download/v#{version}/AI.Toolbox_0.8.5_x64.dmg",
        verified: "github.com/coulsontl/ai-toolbox/"
  end

  name "AI Toolbox"
  desc "Desktop toolbox for managing AI coding assistant configurations"
  homepage "https://github.com/coulsontl/ai-toolbox"

  app "AI Toolbox.app"
end
