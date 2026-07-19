cask "ai-toolbox" do
  version "1.0.6"

  on_arm do
    sha256 "3ddc278b824b74afe59fbc40a2a593dfafbd7a7c46b644d8d8e73ce11c52cdb8"
    url "https://github.com/coulsontl/ai-toolbox/releases/download/v#{version}/AI.Toolbox_1.0.6_aarch64.dmg",
        verified: "github.com/coulsontl/ai-toolbox/"
  end

  on_intel do
    sha256 "38f6b2dcada9bfe54582919310e94ddabdfeec51026c79071438181a88e6b913"
    url "https://github.com/coulsontl/ai-toolbox/releases/download/v#{version}/AI.Toolbox_1.0.6_x64.dmg",
        verified: "github.com/coulsontl/ai-toolbox/"
  end

  name "AI Toolbox"
  desc "Desktop toolbox for managing AI coding assistant configurations"
  homepage "https://github.com/coulsontl/ai-toolbox"

  app "AI Toolbox.app"
end
