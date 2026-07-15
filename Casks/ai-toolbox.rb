cask "ai-toolbox" do
  version "1.0.4"

  on_arm do
    sha256 "4c0619a8bf1eb4f05b6cdc3f1dae0d64c1a3dfd3d671b0d4ccf89adcdcfc6075"
    url "https://github.com/coulsontl/ai-toolbox/releases/download/v#{version}/AI.Toolbox_1.0.4_aarch64.dmg",
        verified: "github.com/coulsontl/ai-toolbox/"
  end

  on_intel do
    sha256 "a7d1553f4a16b8819e0355069a60ff683f1a8ae1cc5731d79eb943da0265ac17"
    url "https://github.com/coulsontl/ai-toolbox/releases/download/v#{version}/AI.Toolbox_1.0.4_x64.dmg",
        verified: "github.com/coulsontl/ai-toolbox/"
  end

  name "AI Toolbox"
  desc "Desktop toolbox for managing AI coding assistant configurations"
  homepage "https://github.com/coulsontl/ai-toolbox"

  app "AI Toolbox.app"
end
