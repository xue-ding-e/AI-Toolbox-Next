cask "ai-toolbox" do
  version "0.8.6"

  on_arm do
    sha256 "39d926057f94063321b4114904cbacbb1bfd9e24137f031e4da81eca188193a4"
    url "https://github.com/coulsontl/ai-toolbox/releases/download/v#{version}/AI.Toolbox_0.8.6_aarch64.dmg",
        verified: "github.com/coulsontl/ai-toolbox/"
  end

  on_intel do
    sha256 "e457f04386e3e334d02686848396cf5a05600fe0e08d9e2fbb4930e073f7ced9"
    url "https://github.com/coulsontl/ai-toolbox/releases/download/v#{version}/AI.Toolbox_0.8.6_x64.dmg",
        verified: "github.com/coulsontl/ai-toolbox/"
  end

  name "AI Toolbox"
  desc "Desktop toolbox for managing AI coding assistant configurations"
  homepage "https://github.com/coulsontl/ai-toolbox"

  app "AI Toolbox.app"
end
