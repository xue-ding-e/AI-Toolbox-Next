cask "ai-toolbox" do
  version "0.8.0"

  on_arm do
    sha256 "46a0154409ed5c15f9f4f249237ff4653a09ce679cd00c0d652bc9600a6deb17"
    url "https://github.com/coulsontl/ai-toolbox/releases/download/v#{version}/AI.Toolbox_0.8.0_aarch64.dmg",
        verified: "github.com/coulsontl/ai-toolbox/"
  end

  on_intel do
    sha256 "a6b096e1cb597fec8b5136b3b1e3ce7d835756f873ed79b5a97260c10b7a69d1"
    url "https://github.com/coulsontl/ai-toolbox/releases/download/v#{version}/AI.Toolbox_0.8.0_x64.dmg",
        verified: "github.com/coulsontl/ai-toolbox/"
  end

  name "AI Toolbox"
  desc "Desktop toolbox for managing AI coding assistant configurations"
  homepage "https://github.com/coulsontl/ai-toolbox"

  app "AI Toolbox.app"
end
