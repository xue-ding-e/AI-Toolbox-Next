cask "ai-toolbox" do
  version "1.1.1"

  on_arm do
    sha256 "476e188cf80491a81700829f8d63133af1148a7412033919fc598ed6be411513"
    url "https://github.com/coulsontl/ai-toolbox/releases/download/v#{version}/AI.Toolbox_1.1.1_aarch64.dmg",
        verified: "github.com/coulsontl/ai-toolbox/"
  end

  on_intel do
    sha256 "2c64912f4b057ef7532ce5eeeac79b109aaee8729ba1478e40c415d7c3d8970a"
    url "https://github.com/coulsontl/ai-toolbox/releases/download/v#{version}/AI.Toolbox_1.1.1_x64.dmg",
        verified: "github.com/coulsontl/ai-toolbox/"
  end

  name "AI Toolbox"
  desc "Desktop toolbox for managing AI coding assistant configurations"
  homepage "https://github.com/coulsontl/ai-toolbox"

  app "AI Toolbox.app"
end
