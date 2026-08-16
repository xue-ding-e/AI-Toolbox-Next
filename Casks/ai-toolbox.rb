cask "ai-toolbox" do
  version "1.1.2"

  on_arm do
    sha256 "ef712fb0367fc1571b4bfd4ed830c111caf3dae67eb1591c5c3c42f62dc5e0a3"
    url "https://github.com/coulsontl/ai-toolbox/releases/download/v#{version}/AI.Toolbox_1.1.2_aarch64.dmg",
        verified: "github.com/coulsontl/ai-toolbox/"
  end

  on_intel do
    sha256 "ac4e005b1fb74df373b31b55bc0516c4c45e6af1df773afe9fe3f098b33d94b5"
    url "https://github.com/coulsontl/ai-toolbox/releases/download/v#{version}/AI.Toolbox_1.1.2_x64.dmg",
        verified: "github.com/coulsontl/ai-toolbox/"
  end

  name "AI Toolbox"
  desc "Desktop toolbox for managing AI coding assistant configurations"
  homepage "https://github.com/coulsontl/ai-toolbox"

  app "AI Toolbox.app"
end
