cask "ai-toolbox" do
  version "0.9.5"

  on_arm do
    sha256 "02bb2159195d8fbe822f87c71f38d49e96190743230a16a9476bb1e69b98604f"
    url "https://github.com/coulsontl/ai-toolbox/releases/download/v#{version}/AI.Toolbox_0.9.5_aarch64.dmg",
        verified: "github.com/coulsontl/ai-toolbox/"
  end

  on_intel do
    sha256 "68428b1dd2fcf945fb03a7019fd2b9364f0330499d88dee5eca6f202b629b023"
    url "https://github.com/coulsontl/ai-toolbox/releases/download/v#{version}/AI.Toolbox_0.9.5_x64.dmg",
        verified: "github.com/coulsontl/ai-toolbox/"
  end

  name "AI Toolbox"
  desc "Desktop toolbox for managing AI coding assistant configurations"
  homepage "https://github.com/coulsontl/ai-toolbox"

  app "AI Toolbox.app"
end
