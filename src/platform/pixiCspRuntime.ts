// PixiJS uses this side effect to replace new Function()-based synchronizers
// with static implementations when the host CSP forbids unsafe-eval.
import "pixi.js/unsafe-eval";
import { Assets } from "pixi.js";

Assets.setPreferences({
  preferWorkers: false,
});
