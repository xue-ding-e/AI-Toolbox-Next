cask "ai-toolbox" do
  version "0.9.7"

  on_arm do
    sha256 "16a715f218f2c21071ef60a10b832fa4e534c26ac1346261ede8663d5814f75d"
    url "https://github.com/coulsontl/ai-toolbox/releases/download/v#{version}/AI.Toolbox_0.9.7_aarch64.dmg",
        verified: "github.com/coulsontl/ai-toolbox/"
  end

  on_intel do
    sha256 "baff2fc924cc19981f53c91b6020a4c65faca8baf044df1210c3aa37b5c55243"
    url "https://github.com/coulsontl/ai-toolbox/releases/download/v#{version}/AI.Toolbox_0.9.7_x64.dmg",
        verified: "github.com/coulsontl/ai-toolbox/"
  end

  name "AI Toolbox"
  desc "Desktop toolbox for managing AI coding assistant configurations"
  homepage "https://github.com/coulsontl/ai-toolbox"

  app "AI Toolbox.app"
end
