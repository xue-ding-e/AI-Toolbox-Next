cask "ai-toolbox" do
  version "1.1.6"

  on_arm do
    sha256 "7975154514a31ecfc58cdb8483d992baab6522fc55056d1347de7b0e6f2434c1"
    url "https://github.com/coulsontl/ai-toolbox/releases/download/v#{version}/AI.Toolbox_1.1.6_aarch64.dmg"
  end

  on_intel do
    sha256 "e65b4342f4dfc1fea9105de7414292202f04c0a52b63066fc6c61d6a3f3f274d"
    url "https://github.com/coulsontl/ai-toolbox/releases/download/v#{version}/AI.Toolbox_1.1.6_x64.dmg"
  end

  name "AI Toolbox"
  desc "Desktop toolbox for managing AI coding assistant configurations"
  homepage "https://github.com/coulsontl/ai-toolbox"
  license "AGPL-3.0-or-later"

  app "AI Toolbox.app"
end
