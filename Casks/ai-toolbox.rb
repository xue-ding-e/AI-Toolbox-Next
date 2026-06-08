cask "ai-toolbox" do
  version "0.9.5"

  on_arm do
    sha256 "c12c14f564cbefa71811db11be0f27c2be5e961c4758cc424675b665932c8fb1"
    url "https://github.com/coulsontl/ai-toolbox/releases/download/v#{version}/AI.Toolbox_0.9.5_aarch64.dmg",
        verified: "github.com/coulsontl/ai-toolbox/"
  end

  on_intel do
    sha256 "856a450978b8829e2b9a97bc00895cfe5592a3d0c79b9eeb95b247ceb9f2ceed"
    url "https://github.com/coulsontl/ai-toolbox/releases/download/v#{version}/AI.Toolbox_0.9.5_x64.dmg",
        verified: "github.com/coulsontl/ai-toolbox/"
  end

  name "AI Toolbox"
  desc "Desktop toolbox for managing AI coding assistant configurations"
  homepage "https://github.com/coulsontl/ai-toolbox"

  app "AI Toolbox.app"
end
