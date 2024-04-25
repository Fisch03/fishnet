// these are macros that are used by the component macro. when used outside, these fake_macros will
// expand to compile_errors
use fishnet_macros::fake_macro;
fake_macro!(state);
fake_macro!(state_init);
