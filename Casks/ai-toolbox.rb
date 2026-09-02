cask "ai-toolbox" do
  version "1.1.4"

  on_arm do
    sha256 "0a5efa71399faf819ad23524dcc6887716b542476e95f403b34a72d75b5e5971"
    url "https://github.com/coulsontl/ai-toolbox/releases/download/v#{version}/AI.Toolbox_1.1.4_aarch64.dmg",
        verified: "github.com/coulsontl/ai-toolbox/"
  end

  on_intel do
    sha256 "a2bbfd23c60df42a3d165fc0fbe698e3940ad3fa7700c9a4494bc031bb7e95b9"
    url "https://github.com/coulsontl/ai-toolbox/releases/download/v#{version}/AI.Toolbox_1.1.4_x64.dmg",
        verified: "github.com/coulsontl/ai-toolbox/"
  end

  name "AI Toolbox"
  desc "Desktop toolbox for managing AI coding assistant configurations"
  homepage "https://github.com/coulsontl/ai-toolbox"

  app "AI Toolbox.app"
end
