cask "ai-toolbox" do
  version "0.9.6"

  on_arm do
    sha256 "789d158e8f287332915a6ed79d3771cc858ffddb24f014004e83a95bcda7921c"
    url "https://github.com/coulsontl/ai-toolbox/releases/download/v#{version}/AI.Toolbox_0.9.6_aarch64.dmg",
        verified: "github.com/coulsontl/ai-toolbox/"
  end

  on_intel do
    sha256 "1eb4ce3ff052598caac2333e3417ec0bbfa0345900e4e669a698d5f03ba794fa"
    url "https://github.com/coulsontl/ai-toolbox/releases/download/v#{version}/AI.Toolbox_0.9.6_x64.dmg",
        verified: "github.com/coulsontl/ai-toolbox/"
  end

  name "AI Toolbox"
  desc "Desktop toolbox for managing AI coding assistant configurations"
  homepage "https://github.com/coulsontl/ai-toolbox"

  app "AI Toolbox.app"
end
