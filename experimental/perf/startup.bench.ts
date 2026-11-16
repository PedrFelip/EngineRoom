import { bench, native } from "@hona/strobe";
import workspace from "./data/workspace-15.json";

export default bench({
  targets: {
    eager: native("eager", { output: true }),
    deferred: native("deferred", { output: true }),
    eager15: native("eager", { output: true, files: { "data/engineroom-experimental/workspace.json": workspace } }),
    deferred15: native("deferred", { output: true, files: { "data/engineroom-experimental/workspace.json": workspace } }),
  },
  cases: {
    "workspace-ready": {
      primary: "launch.ready",
      metrics: ["cpu.work", "memory.app.private", "launch.frame"],
      launch: {
        until: { milestone: "up" },
        milestones: {
          up: { match: "ENGINE_ROOM_READY", in: "output" },
          frame: { match: "ENGINE_ROOM_FIRST_FRAME", in: "output" },
        },
        screen: false,
        timeout: 30_000,
      },
    },
  },
});
