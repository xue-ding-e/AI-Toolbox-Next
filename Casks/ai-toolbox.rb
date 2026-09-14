cask "ai-toolbox" do
  version "1.1.5"

  on_arm do
    sha256 "94590fad777de19f5188aa88bf957ccb5ba05451c6613c75d336426ea94aebba"
    url "https://github.com/coulsontl/ai-toolbox/releases/download/v#{version}/AI.Toolbox_1.1.5_aarch64.dmg"
  end

  on_intel do
    sha256 "214e198495baebf94fd2a89ad09d89fd9a41247aa82e4e1aaf1ccea29425c78e"
    url "https://github.com/coulsontl/ai-toolbox/releases/download/v#{version}/AI.Toolbox_1.1.5_x64.dmg"
  end

  name "AI Toolbox"
  desc "Desktop toolbox for managing AI coding assistant configurations"
  homepage "https://github.com/coulsontl/ai-toolbox"

  app "AI Toolbox.app"
end
