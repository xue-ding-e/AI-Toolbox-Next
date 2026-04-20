cask "ai-toolbox" do
  version "0.8.3"

  on_arm do
    sha256 "110963e0ff9d1c67c1f8036a8821bb0eae81d73a6d6f823d7fc385bf1fdcc9e8"
    url "https://github.com/coulsontl/ai-toolbox/releases/download/v#{version}/AI.Toolbox_0.8.3_aarch64.dmg",
        verified: "github.com/coulsontl/ai-toolbox/"
  end

  on_intel do
    sha256 "42b8a6e4dad054108628decdd8e84b07f295abbb06644188fd4dccc9293b9e7f"
    url "https://github.com/coulsontl/ai-toolbox/releases/download/v#{version}/AI.Toolbox_0.8.3_x64.dmg",
        verified: "github.com/coulsontl/ai-toolbox/"
  end

  name "AI Toolbox"
  desc "Desktop toolbox for managing AI coding assistant configurations"
  homepage "https://github.com/coulsontl/ai-toolbox"

  app "AI Toolbox.app"
end
