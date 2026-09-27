cask "ai-toolbox" do
  version "1.1.8"

  on_arm do
    sha256 "02c8b7c4c29f0ccf061f460813af8c512991666a6019f891b30f165447ed1d33"
    url "https://github.com/coulsontl/ai-toolbox/releases/download/v#{version}/AI.Toolbox_1.1.8_aarch64.dmg"
  end

  on_intel do
    sha256 "ac3e470de9a15398247a50a10ac23f376d4f31cd43a685b19a4ef1e3c17a796a"
    url "https://github.com/coulsontl/ai-toolbox/releases/download/v#{version}/AI.Toolbox_1.1.8_x64.dmg"
  end

  name "AI Toolbox"
  desc "Desktop toolbox for managing AI coding assistant configurations"
  homepage "https://github.com/coulsontl/ai-toolbox"

  app "AI Toolbox.app"
end
