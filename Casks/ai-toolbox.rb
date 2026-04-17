cask "ai-toolbox" do
  version "0.8.2"

  on_arm do
    sha256 "11d4b192f77373547d0d50d0d037ae82d3b86f11d4f95c2b6b755554ec04e0f1"
    url "https://github.com/coulsontl/ai-toolbox/releases/download/v#{version}/AI.Toolbox_0.8.2_aarch64.dmg",
        verified: "github.com/coulsontl/ai-toolbox/"
  end

  on_intel do
    sha256 "01e5985be7336ecf8ae19f758cfc76f31f35dd41b21f90957cf362ce19f30f87"
    url "https://github.com/coulsontl/ai-toolbox/releases/download/v#{version}/AI.Toolbox_0.8.2_x64.dmg",
        verified: "github.com/coulsontl/ai-toolbox/"
  end

  name "AI Toolbox"
  desc "Desktop toolbox for managing AI coding assistant configurations"
  homepage "https://github.com/coulsontl/ai-toolbox"

  app "AI Toolbox.app"
end
