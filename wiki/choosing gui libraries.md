
# GUI engines

# Rust

https://areweguiyet.com/

|     library      |     Support     |                            Notes                             |
| :--------------: | :-------------: | :----------------------------------------------------------: |
|       azul       |   ❌ abandoned   |                                                              |
|      conrod      |    ✔ stable     | ugly default theme, ✔ extensive theming support + gfx compat |
|      druid       |     ⚠ young     |         ✔ good enough default theme + custom themes          |
|       fltk       |  ❗ very young   |           ugly default theme, ⚠ very basic theming           |
|       gtk        |  ✔ very stable  |                 ❗ hard to compile on windows                 |
|       iced       |     ⚠ young     |               ✔ good default theme, dark theme               |
|     imgui-rs     |  ✔ very stable  |                        ❗ ugly themes                         |
|       kas        |     ⚠ young     |                      ugly default theme                      |
|     neutrino     |   ❌ abandoned   |                        ✔ CSS styling                         |
|      orbtk       |    ✔ stable     |         good default theme, ⚠ mobile support planned         |
| qmetaobject (Qt) |  ✔ very stable  |              ⚠ awkward coding (c++ inside rust)              |
|       relm       |  ✔ support ok   |          ⚠ no theming? ❗ hard to compile on windows          |
|   rust-sciter    |  ✔ support ok   |              ✔ electron equivalent, native look              |
|    webrender     | ✔ great support |               ✔ firefox renderer, ⚠ low level                |
|    flutter-rs    |     ⚠ young     |           ⚠ no compatibility with existing plugins           |
|     makepad      | ❌ poor support  |                 ❌ not exactly a GUI library                  |
|      moxie       | ❌ poor support  |                                                              |
|     reclutch     |  ❗ very young   |                         ❌ only core                          |

## Non Rust

Dart (for flutter) and Javascript (for electron) does not have out of the box support for Rust.
The settings schema must be serialized.

* electron:
  * pro: leverage html and javascript experience from contributors
  * cons: slow,
* flutter:
  * pros: faster, great theming, plugins
  * cons: relatively new, fewer developers

## Final choice

Flutter desktop is still unstable.

Begin with iced, switch to dart-Flutter on community demand.
