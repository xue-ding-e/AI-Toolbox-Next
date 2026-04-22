cask "ai-toolbox" do
  version "0.8.4"

  on_arm do
    sha256 "896e1ffd988e448b4dda39e2c147b6f9fc63a1bb7f35f3a06b328ac493f5742f"
    url "https://github.com/coulsontl/ai-toolbox/releases/download/v#{version}/AI.Toolbox_0.8.4_aarch64.dmg",
        verified: "github.com/coulsontl/ai-toolbox/"
  end

  on_intel do
    sha256 "8f9770f7c7d7a75c7d68ea8397ca38160946063f86a02de1da4972d8c4a041b5"
    url "https://github.com/coulsontl/ai-toolbox/releases/download/v#{version}/AI.Toolbox_0.8.4_x64.dmg",
        verified: "github.com/coulsontl/ai-toolbox/"
  end

  name "AI Toolbox"
  desc "Desktop toolbox for managing AI coding assistant configurations"
  homepage "https://github.com/coulsontl/ai-toolbox"

  app "AI Toolbox.app"
end
