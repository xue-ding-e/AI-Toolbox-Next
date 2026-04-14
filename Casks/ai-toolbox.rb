cask "ai-toolbox" do
  version "0.8.1"

  on_arm do
    sha256 "e27b0c1355effcafceab0976166bacb828a1a28505ca334e1cb19b8a8f09e967"
    url "https://github.com/coulsontl/ai-toolbox/releases/download/v#{version}/AI.Toolbox_0.8.1_aarch64.dmg",
        verified: "github.com/coulsontl/ai-toolbox/"
  end

  on_intel do
    sha256 "e2ff9b691ec7ead1157fad6f6e8ae9518ec0803ecbbb38eaa565e62d48b7eced"
    url "https://github.com/coulsontl/ai-toolbox/releases/download/v#{version}/AI.Toolbox_0.8.1_x64.dmg",
        verified: "github.com/coulsontl/ai-toolbox/"
  end

  name "AI Toolbox"
  desc "Desktop toolbox for managing AI coding assistant configurations"
  homepage "https://github.com/coulsontl/ai-toolbox"

  app "AI Toolbox.app"
end
