// The Canvas owns this PixiJS runtime configuration. The static synchronizers
// keep it compatible with the production CSP without leaking host setup into
// the rest of the UI.
import "pixi.js/unsafe-eval";
import { Assets } from "pixi.js";

Assets.setPreferences({
  preferWorkers: false,
});
