# NVIDIA Control Panel but in Linux!?! 🤯
It allows you to run programs on your NVIDIA GPU, without adding extra options before running your executable.
Just add it to the control panel and run you programs as you would normally do!

## How to use
AppImage coming soon, if there's ever someone else using this\
For now compile it yourself!
1. Have Rust installed
2. Clone the repository
3. `cd` into the repository
4. Run `cargo run`

Now just add your application and you're done! It'll use the NVIDIA GPU next time you run it, hopefully.

## How it works
At the moment it is creating wrapper scripts and symlinks them to the executable directly but this may change in the future, I really hate wrapper scripts and those sort of stuff, I just want the feature to be like how it's on windows, it'll run the program on the NVIDIA GPU no matter what, no need to specify any special arguments before it.