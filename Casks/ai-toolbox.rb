cask "ai-toolbox" do
  version "0.9.7"

  on_arm do
    sha256 "780a5dcc6193472d3fef81bf42b7e8fbf4112bbe9b0ab6334bc5695ae3e841c1"
    url "https://github.com/coulsontl/ai-toolbox/releases/download/v#{version}/AI.Toolbox_0.9.7_aarch64.dmg",
        verified: "github.com/coulsontl/ai-toolbox/"
  end

  on_intel do
    sha256 "ed8c20f84997e32c53da4861b2b59e4938199ac6c2d88592f8f8436525968e93"
    url "https://github.com/coulsontl/ai-toolbox/releases/download/v#{version}/AI.Toolbox_0.9.7_x64.dmg",
        verified: "github.com/coulsontl/ai-toolbox/"
  end

  name "AI Toolbox"
  desc "Desktop toolbox for managing AI coding assistant configurations"
  homepage "https://github.com/coulsontl/ai-toolbox"

  app "AI Toolbox.app"
end
