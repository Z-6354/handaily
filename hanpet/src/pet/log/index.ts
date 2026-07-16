export {
  petLog,
  petTimingStart,
  petTimingEnd,
  petTimingMark,
  PetPhaseTimer,
  type PetLogLevel,
  type PetTimingMark,
} from "./petDebugLog";
export {
  initPetLogSink,
  flushPetLogSink,
  formatSinkLine,
  queueSinkLine,
  type PetLogSinkFn,
} from "./petLogSink";
