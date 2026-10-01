cask "ai-toolbox" do
  version "1.1.9"

  on_arm do
    sha256 "ae322907334d8387a2ea35dfd00fbbbee91d4cc0fe1c118ee904cc5e6305aa34"
    url "https://github.com/coulsontl/ai-toolbox/releases/download/v#{version}/AI.Toolbox_1.1.9_aarch64.dmg"
  end

  on_intel do
    sha256 "ee4bbd6b4cff18ea7994ff49533ad0551da71332efae1a69ce585f664a11897f"
    url "https://github.com/coulsontl/ai-toolbox/releases/download/v#{version}/AI.Toolbox_1.1.9_x64.dmg"
  end

  name "AI Toolbox"
  desc "Desktop toolbox for managing AI coding assistant configurations"
  homepage "https://github.com/coulsontl/ai-toolbox"

  app "AI Toolbox.app"
end
