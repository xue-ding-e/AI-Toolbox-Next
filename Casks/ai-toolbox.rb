cask "ai-toolbox" do
  version "0.9.3"

  on_arm do
    sha256 "7b6514a55359c2be2518c6a229ea21d67dcbe0c105c008535e28d0b7110aa970"
    url "https://github.com/coulsontl/ai-toolbox/releases/download/v#{version}/AI.Toolbox_0.9.3_aarch64.dmg",
        verified: "github.com/coulsontl/ai-toolbox/"
  end

  on_intel do
    sha256 "23f06a34b0db1bef822534d58d9871b62ac4682ee13bd17c614c25d0b43852a6"
    url "https://github.com/coulsontl/ai-toolbox/releases/download/v#{version}/AI.Toolbox_0.9.3_x64.dmg",
        verified: "github.com/coulsontl/ai-toolbox/"
  end

  name "AI Toolbox"
  desc "Desktop toolbox for managing AI coding assistant configurations"
  homepage "https://github.com/coulsontl/ai-toolbox"

  app "AI Toolbox.app"
end
