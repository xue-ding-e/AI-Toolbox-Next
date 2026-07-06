cask "ai-toolbox" do
  version "1.0.2"

  on_arm do
    sha256 "166ca83590879c4fa432bbf1831fe8ed8be3b2ebb9c242410f6f8f44437f921a"
    url "https://github.com/coulsontl/ai-toolbox/releases/download/v#{version}/AI.Toolbox_1.0.2_aarch64.dmg",
        verified: "github.com/coulsontl/ai-toolbox/"
  end

  on_intel do
    sha256 "13f39d57a5e6b13d9b4075d79fe260845760b8fad53410b427e7a3ca8784e759"
    url "https://github.com/coulsontl/ai-toolbox/releases/download/v#{version}/AI.Toolbox_1.0.2_x64.dmg",
        verified: "github.com/coulsontl/ai-toolbox/"
  end

  name "AI Toolbox"
  desc "Desktop toolbox for managing AI coding assistant configurations"
  homepage "https://github.com/coulsontl/ai-toolbox"

  app "AI Toolbox.app"
end
