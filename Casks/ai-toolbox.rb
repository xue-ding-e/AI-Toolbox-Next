cask "ai-toolbox" do
  version "1.0.9"

  on_arm do
    sha256 "99da5a367816c9fc9362e2e5cf3ba8f006395efb620a2a9d75ceeb5332d49b7d"
    url "https://github.com/coulsontl/ai-toolbox/releases/download/v#{version}/AI.Toolbox_1.0.9_aarch64.dmg",
        verified: "github.com/coulsontl/ai-toolbox/"
  end

  on_intel do
    sha256 "76df64aaad33c8435c660bb58a4d4f431ba83fed9c4e9ae265e315f593062c7d"
    url "https://github.com/coulsontl/ai-toolbox/releases/download/v#{version}/AI.Toolbox_1.0.9_x64.dmg",
        verified: "github.com/coulsontl/ai-toolbox/"
  end

  name "AI Toolbox"
  desc "Desktop toolbox for managing AI coding assistant configurations"
  homepage "https://github.com/coulsontl/ai-toolbox"

  app "AI Toolbox.app"
end
