cask "ai-toolbox" do
  version "1.1.3"

  on_arm do
    sha256 "c00f419411366ab2e8ee7ce21b2c6c454d01d257d1d881446b5bcda158b02a19"
    url "https://github.com/coulsontl/ai-toolbox/releases/download/v#{version}/AI.Toolbox_1.1.3_aarch64.dmg",
        verified: "github.com/coulsontl/ai-toolbox/"
  end

  on_intel do
    sha256 "bb1c636d09857df2248fdbb0091e84f4660852a5b47bfce0a68d252c7e654322"
    url "https://github.com/coulsontl/ai-toolbox/releases/download/v#{version}/AI.Toolbox_1.1.3_x64.dmg",
        verified: "github.com/coulsontl/ai-toolbox/"
  end

  name "AI Toolbox"
  desc "Desktop toolbox for managing AI coding assistant configurations"
  homepage "https://github.com/coulsontl/ai-toolbox"

  app "AI Toolbox.app"
end
