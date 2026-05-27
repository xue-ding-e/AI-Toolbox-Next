cask "ai-toolbox" do
  version "0.9.2"

  on_arm do
    sha256 "596de999a6cd18261c680793b8fdeb11e6ba33fdde929bcf8e3d4ec7993f8d67"
    url "https://github.com/coulsontl/ai-toolbox/releases/download/v#{version}/AI.Toolbox_0.9.2_aarch64.dmg",
        verified: "github.com/coulsontl/ai-toolbox/"
  end

  on_intel do
    sha256 "02e5a861427f9ee99650b805de130a9df2bc4e691da56c9eea5506cbb2946f77"
    url "https://github.com/coulsontl/ai-toolbox/releases/download/v#{version}/AI.Toolbox_0.9.2_x64.dmg",
        verified: "github.com/coulsontl/ai-toolbox/"
  end

  name "AI Toolbox"
  desc "Desktop toolbox for managing AI coding assistant configurations"
  homepage "https://github.com/coulsontl/ai-toolbox"

  app "AI Toolbox.app"
end
