# fishnet
born from my growing hate for most modern web frameworks , fishnet is my take on combining old with new: 
a highly performant component based web framework for building quick, open (both for you and the end user) websites.

you should use fishnet if you want:
- 0% hydration, resumability, client side routing, virtual doms - 100% pure html, css and js *you wrote* beamed straight from server to client, mostly limited by whatever speed your network is capable of :)
- first-class support for [htmx](https://htmx.org/) in the form of easily defined per-component api endpoints

you probably **shouldn't** use fishnet if:
- you are building anything bigger than a personal website or side project
- you want a highly dynamic page (fishnet can dynamically render components on the server, but this should be used as sparingly as possible)
- you aren't fine with using a immature library developed by an idiot that may or may not be maintained in the future

## using fishnet
don't. at least for now. it's still undergoing heavy development in parallel to [my upcoming new website](https://github.com/Fisch03/sakanaa.moe). 
i'm glad that you're apparently interested enough to have read this far and would be happy to see you try it out once its ready!

if you want to play around with the codebase nonetheless, add it to your `Cargo.toml` using
```toml
fishnet = { git = "https://github.com/Fisch03/fishnet.git" }
```
docs are available under https://fisch03.xyz/fishnet/ (for now). have fun!
