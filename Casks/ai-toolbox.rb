cask "ai-toolbox" do
  version "1.0.1"

  on_arm do
    sha256 "6c290888488041737a3c2662a7e4cc9fb7f4293786257326f350845c73bb4f0a"
    url "https://github.com/coulsontl/ai-toolbox/releases/download/v#{version}/AI.Toolbox_1.0.1_aarch64.dmg",
        verified: "github.com/coulsontl/ai-toolbox/"
  end

  on_intel do
    sha256 "d169df46cbfe5dca73e507b81ff1a5a1010873d988688685f97db639c9a18ea0"
    url "https://github.com/coulsontl/ai-toolbox/releases/download/v#{version}/AI.Toolbox_1.0.1_x64.dmg",
        verified: "github.com/coulsontl/ai-toolbox/"
  end

  name "AI Toolbox"
  desc "Desktop toolbox for managing AI coding assistant configurations"
  homepage "https://github.com/coulsontl/ai-toolbox"

  app "AI Toolbox.app"
end
