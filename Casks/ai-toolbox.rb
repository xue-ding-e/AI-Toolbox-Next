cask "ai-toolbox" do
  version "0.9.4"

  on_arm do
    sha256 "95f70f5f76dee3b06467d75c6a5326a434d1f0dc33c40e23308ee5094feb1c6d"
    url "https://github.com/coulsontl/ai-toolbox/releases/download/v#{version}/AI.Toolbox_0.9.4_aarch64.dmg",
        verified: "github.com/coulsontl/ai-toolbox/"
  end

  on_intel do
    sha256 "faa264e050101ca621eee62de71a6237330530a3a400d953cf1dad852424ab50"
    url "https://github.com/coulsontl/ai-toolbox/releases/download/v#{version}/AI.Toolbox_0.9.4_x64.dmg",
        verified: "github.com/coulsontl/ai-toolbox/"
  end

  name "AI Toolbox"
  desc "Desktop toolbox for managing AI coding assistant configurations"
  homepage "https://github.com/coulsontl/ai-toolbox"

  app "AI Toolbox.app"
end
