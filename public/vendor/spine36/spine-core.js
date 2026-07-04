var __extends = (this && this.__extends) || (function () {
	var extendStatics = Object.setPrototypeOf ||
		({ __proto__: [] } instanceof Array && function (d, b) { d.__proto__ = b; }) ||
		function (d, b) { for (var p in b) if (b.hasOwnProperty(p)) d[p] = b[p]; };
	return function (d, b) {
		extendStatics(d, b);
		function __() { this.constructor = d; }
		d.prototype = b === null ? Object.create(b) : (__.prototype = b.prototype, new __());
	};
})();
var spine;
(function (spine) {
	var Animation = (function () {
		function Animation(name, timelines, duration) {
			if (name == null)
				throw new Error("name cannot be null.");
			if (timelines == null)
				throw new Error("timelines cannot be null.");
			this.name = name;
			this.timelines = timelines;
			this.duration = duration;
		}
		Animation.prototype.apply = function (skeleton, lastTime, time, loop, events, alpha, pose, direction) {
			if (skeleton == null)
				throw new Error("skeleton cannot be null.");
			if (loop && this.duration != 0) {
				time %= this.duration;
				if (lastTime > 0)
					lastTime %= this.duration;
			}
			var timelines = this.timelines;
			for (var i = 0, n = timelines.length; i < n; i++)
				timelines[i].apply(skeleton, lastTime, time, events, alpha, pose, direction);
		};
		Animation.binarySearch = function (values, target, step) {
			if (step === void 0) { step = 1; }
			var low = 0;
			var high = values.length / step - 2;
			if (high == 0)
				return step;
			var current = high >>> 1;
			while (true) {
				if (values[(current + 1) * step] <= target)
					low = current + 1;
				else
					high = current;
				if (low == high)
					return (low + 1) * step;
				current = (low + high) >>> 1;
			}
		};
		Animation.linearSearch = function (values, target, step) {
			for (var i = 0, last = values.length - step; i <= last; i += step)
				if (values[i] > target)
					return i;
			return -1;
		};
		return Animation;
	}());
	spine.Animation = Animation;
	var MixPose;
	(function (MixPose) {
		MixPose[MixPose["setup"] = 0] = "setup";
		MixPose[MixPose["current"] = 1] = "current";
		MixPose[MixPose["currentLayered"] = 2] = "currentLayered";
	})(MixPose = spine.MixPose || (spine.MixPose = {}));
	var MixDirection;
	(function (MixDirection) {
		MixDirection[MixDirection["in"] = 0] = "in";
		MixDirection[MixDirection["out"] = 1] = "out";
	})(MixDirection = spine.MixDirection || (spine.MixDirection = {}));
	var TimelineType;
	(function (TimelineType) {
		TimelineType[TimelineType["rotate"] = 0] = "rotate";
		TimelineType[TimelineType["translate"] = 1] = "translate";
		TimelineType[TimelineType["scale"] = 2] = "scale";
		TimelineType[TimelineType["shear"] = 3] = "shear";
		TimelineType[TimelineType["attachment"] = 4] = "attachment";
		TimelineType[TimelineType["color"] = 5] = "color";
		TimelineType[TimelineType["deform"] = 6] = "deform";
		TimelineType[TimelineType["event"] = 7] = "event";
		TimelineType[TimelineType["drawOrder"] = 8] = "drawOrder";
		TimelineType[TimelineType["ikConstraint"] = 9] = "ikConstraint";
		TimelineType[TimelineType["transformConstraint"] = 10] = "transformConstraint";
		TimelineType[TimelineType["pathConstraintPosition"] = 11] = "pathConstraintPosition";
		TimelineType[TimelineType["pathConstraintSpacing"] = 12] = "pathConstraintSpacing";
		TimelineType[TimelineType["pathConstraintMix"] = 13] = "pathConstraintMix";
		TimelineType[TimelineType["twoColor"] = 14] = "twoColor";
	})(TimelineType = spine.TimelineType || (spine.TimelineType = {}));
	var CurveTimeline = (function () {
		function CurveTimeline(frameCount) {
			if (frameCount <= 0)
				throw new Error("frameCount must be > 0: " + frameCount);
			this.curves = spine.Utils.newFloatArray((frameCount - 1) * CurveTimeline.BEZIER_SIZE);
		}
		CurveTimeline.prototype.getFrameCount = function () {
			return this.curves.length / CurveTimeline.BEZIER_SIZE + 1;
		};
		CurveTimeline.prototype.setLinear = function (frameIndex) {
			this.curves[frameIndex * CurveTimeline.BEZIER_SIZE] = CurveTimeline.LINEAR;
		};
		CurveTimeline.prototype.setStepped = function (frameIndex) {
			this.curves[frameIndex * CurveTimeline.BEZIER_SIZE] = CurveTimeline.STEPPED;
		};
		CurveTimeline.prototype.getCurveType = function (frameIndex) {
			var index = frameIndex * CurveTimeline.BEZIER_SIZE;
			if (index == this.curves.length)
				return CurveTimeline.LINEAR;
			var type = this.curves[index];
			if (type == CurveTimeline.LINEAR)
				return CurveTimeline.LINEAR;
			if (type == CurveTimeline.STEPPED)
				return CurveTimeline.STEPPED;
			return CurveTimeline.BEZIER;
		};
		CurveTimeline.prototype.setCurve = function (frameIndex, cx1, cy1, cx2, cy2) {
			var tmpx = (-cx1 * 2 + cx2) * 0.03, tmpy = (-cy1 * 2 + cy2) * 0.03;
			var dddfx = ((cx1 - cx2) * 3 + 1) * 0.006, dddfy = ((cy1 - cy2) * 3 + 1) * 0.006;
			var ddfx = tmpx * 2 + dddfx, ddfy = tmpy * 2 + dddfy;
			var dfx = cx1 * 0.3 + tmpx + dddfx * 0.16666667, dfy = cy1 * 0.3 + tmpy + dddfy * 0.16666667;
			var i = frameIndex * CurveTimeline.BEZIER_SIZE;
			var curves = this.curves;
			curves[i++] = CurveTimeline.BEZIER;
			var x = dfx, y = dfy;
			for (var n = i + CurveTimeline.BEZIER_SIZE - 1; i < n; i += 2) {
				curves[i] = x;
				curves[i + 1] = y;
				dfx += ddfx;
				dfy += ddfy;
				ddfx += dddfx;
				ddfy += dddfy;
				x += dfx;
				y += dfy;
			}
		};
		CurveTimeline.prototype.getCurvePercent = function (frameIndex, percent) {
			percent = spine.MathUtils.clamp(percent, 0, 1);
			var curves = this.curves;
			var i = frameIndex * CurveTimeline.BEZIER_SIZE;
			var type = curves[i];
			if (type == CurveTimeline.LINEAR)
				return percent;
			if (type == CurveTimeline.STEPPED)
				return 0;
			i++;
			var x = 0;
			for (var start = i, n = i + CurveTimeline.BEZIER_SIZE - 1; i < n; i += 2) {
				x = curves[i];
				if (x >= percent) {
					var prevX = void 0, prevY = void 0;
					if (i == start) {
						prevX = 0;
						prevY = 0;
					}
					else {
						prevX = curves[i - 2];
						prevY = curves[i - 1];
					}
					return prevY + (curves[i + 1] - prevY) * (percent - prevX) / (x - prevX);
				}
			}
			var y = curves[i - 1];
			return y + (1 - y) * (percent - x) / (1 - x);
		};
		CurveTimeline.LINEAR = 0;
		CurveTimeline.STEPPED = 1;
		CurveTimeline.BEZIER = 2;
		CurveTimeline.BEZIER_SIZE = 10 * 2 - 1;
		return CurveTimeline;
	}());
	spine.CurveTimeline = CurveTimeline;
	var RotateTimeline = (function (_super) {
		__extends(RotateTimeline, _super);
		function RotateTimeline(frameCount) {
			var _this = _super.call(this, frameCount) || this;
			_this.frames = spine.Utils.newFloatArray(frameCount << 1);
			return _this;
		}
		RotateTimeline.prototype.getPropertyId = function () {
			return (TimelineType.rotate << 24) + this.boneIndex;
		};
		RotateTimeline.prototype.setFrame = function (frameIndex, time, degrees) {
			frameIndex <<= 1;
			this.frames[frameIndex] = time;
			this.frames[frameIndex + RotateTimeline.ROTATION] = degrees;
		};
		RotateTimeline.prototype.apply = function (skeleton, lastTime, time, events, alpha, pose, direction) {
			var frames = this.frames;
			var bone = skeleton.bones[this.boneIndex];
			if (time < frames[0]) {
				switch (pose) {
					case MixPose.setup:
						bone.rotation = bone.data.rotation;
						return;
					case MixPose.current:
						var r_1 = bone.data.rotation - bone.rotation;
						r_1 -= (16384 - ((16384.499999999996 - r_1 / 360) | 0)) * 360;
						bone.rotation += r_1 * alpha;
				}
				return;
			}
			if (time >= frames[frames.length - RotateTimeline.ENTRIES]) {
				if (pose == MixPose.setup)
					bone.rotation = bone.data.rotation + frames[frames.length + RotateTimeline.PREV_ROTATION] * alpha;
				else {
					var r_2 = bone.data.rotation + frames[frames.length + RotateTimeline.PREV_ROTATION] - bone.rotation;
					r_2 -= (16384 - ((16384.499999999996 - r_2 / 360) | 0)) * 360;
					bone.rotation += r_2 * alpha;
				}
				return;
			}
			var frame = Animation.binarySearch(frames, time, RotateTimeline.ENTRIES);
			var prevRotation = frames[frame + RotateTimeline.PREV_ROTATION];
			var frameTime = frames[frame];
			var percent = this.getCurvePercent((frame >> 1) - 1, 1 - (time - frameTime) / (frames[frame + RotateTimeline.PREV_TIME] - frameTime));
			var r = frames[frame + RotateTimeline.ROTATION] - prevRotation;
			r -= (16384 - ((16384.499999999996 - r / 360) | 0)) * 360;
			r = prevRotation + r * percent;
			if (pose == MixPose.setup) {
				r -= (16384 - ((16384.499999999996 - r / 360) | 0)) * 360;
				bone.rotation = bone.data.rotation + r * alpha;
			}
			else {
				r = bone.data.rotation + r - bone.rotation;
				r -= (16384 - ((16384.499999999996 - r / 360) | 0)) * 360;
				bone.rotation += r * alpha;
			}
		};
		RotateTimeline.ENTRIES = 2;
		RotateTimeline.PREV_TIME = -2;
		RotateTimeline.PREV_ROTATION = -1;
		RotateTimeline.ROTATION = 1;
		return RotateTimeline;
	}(CurveTimeline));
	spine.RotateTimeline = RotateTimeline;
	var TranslateTimeline = (function (_super) {
		__extends(TranslateTimeline, _super);
		function TranslateTimeline(frameCount) {
			var _this = _super.call(this, frameCount) || this;
			_this.frames = spine.Utils.newFloatArray(frameCount * TranslateTimeline.ENTRIES);
			return _this;
		}
		TranslateTimeline.prototype.getPropertyId = function () {
			return (TimelineType.translate << 24) + this.boneIndex;
		};
		TranslateTimeline.prototype.setFrame = function (frameIndex, time, x, y) {
			frameIndex *= TranslateTimeline.ENTRIES;
			this.frames[frameIndex] = time;
			this.frames[frameIndex + TranslateTimeline.X] = x;
			this.frames[frameIndex + TranslateTimeline.Y] = y;
		};
		TranslateTimeline.prototype.apply = function (skeleton, lastTime, time, events, alpha, pose, direction) {
			var frames = this.frames;
			var bone = skeleton.bones[this.boneIndex];
			if (time < frames[0]) {
				switch (pose) {
					case MixPose.setup:
						bone.x = bone.data.x;
						bone.y = bone.data.y;
						return;
					case MixPose.current:
						bone.x += (bone.data.x - bone.x) * alpha;
						bone.y += (bone.data.y - bone.y) * alpha;
				}
				return;
			}
			var x = 0, y = 0;
			if (time >= frames[frames.length - TranslateTimeline.ENTRIES]) {
				x = frames[frames.length + TranslateTimeline.PREV_X];
				y = frames[frames.length + TranslateTimeline.PREV_Y];
			}
			else {
				var frame = Animation.binarySearch(frames, time, TranslateTimeline.ENTRIES);
				x = frames[frame + TranslateTimeline.PREV_X];
				y = frames[frame + TranslateTimeline.PREV_Y];
				var frameTime = frames[frame];
				var percent = this.getCurvePercent(frame / TranslateTimeline.ENTRIES - 1, 1 - (time - frameTime) / (frames[frame + TranslateTimeline.PREV_TIME] - frameTime));
				x += (frames[frame + TranslateTimeline.X] - x) * percent;
				y += (frames[frame + TranslateTimeline.Y] - y) * percent;
			}
			if (pose == MixPose.setup) {
				bone.x = bone.data.x + x * alpha;
				bone.y = bone.data.y + y * alpha;
			}
			else {
				bone.x += (bone.data.x + x - bone.x) * alpha;
				bone.y += (bone.data.y + y - bone.y) * alpha;
			}
		};
		TranslateTimeline.ENTRIES = 3;
		TranslateTimeline.PREV_TIME = -3;
		TranslateTimeline.PREV_X = -2;
		TranslateTimeline.PREV_Y = -1;
		TranslateTimeline.X = 1;
		TranslateTimeline.Y = 2;
		return TranslateTimeline;
	}(CurveTimeline));
	spine.TranslateTimeline = TranslateTimeline;
	var ScaleTimeline = (function (_super) {
		__extends(ScaleTimeline, _super);
		function ScaleTimeline(frameCount) {
			return _super.call(this, frameCount) || this;
		}
		ScaleTimeline.prototype.getPropertyId = function () {
			return (TimelineType.scale << 24) + this.boneIndex;
		};
		ScaleTimeline.prototype.apply = function (skeleton, lastTime, time, events, alpha, pose, direction) {
			var frames = this.frames;
			var bone = skeleton.bones[this.boneIndex];
			if (time < frames[0]) {
				switch (pose) {
					case MixPose.setup:
						bone.scaleX = bone.data.scaleX;
						bone.scaleY = bone.data.scaleY;
						return;
					case MixPose.current:
						bone.scaleX += (bone.data.scaleX - bone.scaleX) * alpha;
						bone.scaleY += (bone.data.scaleY - bone.scaleY) * alpha;
				}
				return;
			}
			var x = 0, y = 0;
			if (time >= frames[frames.length - ScaleTimeline.ENTRIES]) {
				x = frames[frames.length + ScaleTimeline.PREV_X] * bone.data.scaleX;
				y = frames[frames.length + ScaleTimeline.PREV_Y] * bone.data.scaleY;
			}
			else {
				var frame = Animation.binarySearch(frames, time, ScaleTimeline.ENTRIES);
				x = frames[frame + ScaleTimeline.PREV_X];
				y = frames[frame + ScaleTimeline.PREV_Y];
				var frameTime = frames[frame];
				var percent = this.getCurvePercent(frame / ScaleTimeline.ENTRIES - 1, 1 - (time - frameTime) / (frames[frame + ScaleTimeline.PREV_TIME] - frameTime));
				x = (x + (frames[frame + ScaleTimeline.X] - x) * percent) * bone.data.scaleX;
				y = (y + (frames[frame + ScaleTimeline.Y] - y) * percent) * bone.data.scaleY;
			}
			if (alpha == 1) {
				bone.scaleX = x;
				bone.scaleY = y;
			}
			else {
				var bx = 0, by = 0;
				if (pose == MixPose.setup) {
					bx = bone.data.scaleX;
					by = bone.data.scaleY;
				}
				else {
					bx = bone.scaleX;
					by = bone.scaleY;
				}
				if (direction == MixDirection.out) {
					x = Math.abs(x) * spine.MathUtils.signum(bx);
					y = Math.abs(y) * spine.MathUtils.signum(by);
				}
				else {
					bx = Math.abs(bx) * spine.MathUtils.signum(x);
					by = Math.abs(by) * spine.MathUtils.signum(y);
				}
				bone.scaleX = bx + (x - bx) * alpha;
				bone.scaleY = by + (y - by) * alpha;
			}
		};
		return ScaleTimeline;
	}(TranslateTimeline));
	spine.ScaleTimeline = ScaleTimeline;
	var ShearTimeline = (function (_super) {
		__extends(ShearTimeline, _super);
		function ShearTimeline(frameCount) {
			return _super.call(this, frameCount) || this;
		}
		ShearTimeline.prototype.getPropertyId = function () {
			return (TimelineType.shear << 24) + this.boneIndex;
		};
		ShearTimeline.prototype.apply = function (skeleton, lastTime, time, events, alpha, pose, direction) {
			var frames = this.frames;
			var bone = skeleton.bones[this.boneIndex];
			if (time < frames[0]) {
				switch (pose) {
					case MixPose.setup:
						bone.shearX = bone.data.shearX;
						bone.shearY = bone.data.shearY;
						return;
					case MixPose.current:
						bone.shearX += (bone.data.shearX - bone.shearX) * alpha;
						bone.shearY += (bone.data.shearY - bone.shearY) * alpha;
				}
				return;
			}
			var x = 0, y = 0;
			if (time >= frames[frames.length - ShearTimeline.ENTRIES]) {
				x = frames[frames.length + ShearTimeline.PREV_X];
				y = frames[frames.length + ShearTimeline.PREV_Y];
			}
			else {
				var frame = Animation.binarySearch(frames, time, ShearTimeline.ENTRIES);
				x = frames[frame + ShearTimeline.PREV_X];
				y = frames[frame + ShearTimeline.PREV_Y];
				var frameTime = frames[frame];
				var percent = this.getCurvePercent(frame / ShearTimeline.ENTRIES - 1, 1 - (time - frameTime) / (frames[frame + ShearTimeline.PREV_TIME] - frameTime));
				x = x + (frames[frame + ShearTimeline.X] - x) * percent;
				y = y + (frames[frame + ShearTimeline.Y] - y) * percent;
			}
			if (pose == MixPose.setup) {
				bone.shearX = bone.data.shearX + x * alpha;
				bone.shearY = bone.data.shearY + y * alpha;
			}
			else {
				bone.shearX += (bone.data.shearX + x - bone.shearX) * alpha;
				bone.shearY += (bone.data.shearY + y - bone.shearY) * alpha;
			}
		};
		return ShearTimeline;
	}(TranslateTimeline));
	spine.ShearTimeline = ShearTimeline;
	var ColorTimeline = (function (_super) {
		__extends(ColorTimeline, _super);
		function ColorTimeline(frameCount) {
			var _this = _super.call(this, frameCount) || this;
			_this.frames = spine.Utils.newFloatArray(frameCount * ColorTimeline.ENTRIES);
			return _this;
		}
		ColorTimeline.prototype.getPropertyId = function () {
			return (TimelineType.color << 24) + this.slotIndex;
		};
		ColorTimeline.prototype.setFrame = function (frameIndex, time, r, g, b, a) {
			frameIndex *= ColorTimeline.ENTRIES;
			this.frames[frameIndex] = time;
			this.frames[frameIndex + ColorTimeline.R] = r;
			this.frames[frameIndex + ColorTimeline.G] = g;
			this.frames[frameIndex + ColorTimeline.B] = b;
			this.frames[frameIndex + ColorTimeline.A] = a;
		};
		ColorTimeline.prototype.apply = function (skeleton, lastTime, time, events, alpha, pose, direction) {
			var slot = skeleton.slots[this.slotIndex];
			var frames = this.frames;
			if (time < frames[0]) {
				switch (pose) {
					case MixPose.setup:
						slot.color.setFromColor(slot.data.color);
						return;
					case MixPose.current:
						var color = slot.color, setup = slot.data.color;
						color.add((setup.r - color.r) * alpha, (setup.g - color.g) * alpha, (setup.b - color.b) * alpha, (setup.a - color.a) * alpha);
				}
				return;
			}
			var r = 0, g = 0, b = 0, a = 0;
			if (time >= frames[frames.length - ColorTimeline.ENTRIES]) {
				var i = frames.length;
				r = frames[i + ColorTimeline.PREV_R];
				g = frames[i + ColorTimeline.PREV_G];
				b = frames[i + ColorTimeline.PREV_B];
				a = frames[i + ColorTimeline.PREV_A];
			}
			else {
				var frame = Animation.binarySearch(frames, time, ColorTimeline.ENTRIES);
				r = frames[frame + ColorTimeline.PREV_R];
				g = frames[frame + ColorTimeline.PREV_G];
				b = frames[frame + ColorTimeline.PREV_B];
				a = frames[frame + ColorTimeline.PREV_A];
				var frameTime = frames[frame];
				var percent = this.getCurvePercent(frame / ColorTimeline.ENTRIES - 1, 1 - (time - frameTime) / (frames[frame + ColorTimeline.PREV_TIME] - frameTime));
				r += (frames[frame + ColorTimeline.R] - r) * percent;
				g += (frames[frame + ColorTimeline.G] - g) * percent;
				b += (frames[frame + ColorTimeline.B] - b) * percent;
				a += (frames[frame + ColorTimeline.A] - a) * percent;
			}
			if (alpha == 1)
				slot.color.set(r, g, b, a);
			else {
				var color = slot.color;
				if (pose == MixPose.setup)
					color.setFromColor(slot.data.color);
				color.add((r - color.r) * alpha, (g - color.g) * alpha, (b - color.b) * alpha, (a - color.a) * alpha);
			}
		};
		ColorTimeline.ENTRIES = 5;
		ColorTimeline.PREV_TIME = -5;
		ColorTimeline.PREV_R = -4;
		ColorTimeline.PREV_G = -3;
		ColorTimeline.PREV_B = -2;
		ColorTimeline.PREV_A = -1;
		ColorTimeline.R = 1;
		ColorTimeline.G = 2;
		ColorTimeline.B = 3;
		ColorTimeline.A = 4;
		return ColorTimeline;
	}(CurveTimeline));
	spine.ColorTimeline = ColorTimeline;
	var TwoColorTimeline = (function (_super) {
		__extends(TwoColorTimeline, _super);
		function TwoColorTimeline(frameCount) {
			var _this = _super.call(this, frameCount) || this;
			_this.frames = spine.Utils.newFloatArray(frameCount * TwoColorTimeline.ENTRIES);
			return _this;
		}
		TwoColorTimeline.prototype.getPropertyId = function () {
			return (TimelineType.twoColor << 24) + this.slotIndex;
		};
		TwoColorTimeline.prototype.setFrame = function (frameIndex, time, r, g, b, a, r2, g2, b2) {
			frameIndex *= TwoColorTimeline.ENTRIES;
			this.frames[frameIndex] = time;
			this.frames[frameIndex + TwoColorTimeline.R] = r;
			this.frames[frameIndex + TwoColorTimeline.G] = g;
			this.frames[frameIndex + TwoColorTimeline.B] = b;
			this.frames[frameIndex + TwoColorTimeline.A] = a;
			this.frames[frameIndex + TwoColorTimeline.R2] = r2;
			this.frames[frameIndex + TwoColorTimeline.G2] = g2;
			this.frames[frameIndex + TwoColorTimeline.B2] = b2;
		};
		TwoColorTimeline.prototype.apply = function (skeleton, lastTime, time, events, alpha, pose, direction) {
			var slot = skeleton.slots[this.slotIndex];
			var frames = this.frames;
			if (time < frames[0]) {
				switch (pose) {
					case MixPose.setup:
						slot.color.setFromColor(slot.data.color);
						slot.darkColor.setFromColor(slot.data.darkColor);
						return;
					case MixPose.current:
						var light = slot.color, dark = slot.darkColor, setupLight = slot.data.color, setupDark = slot.data.darkColor;
						light.add((setupLight.r - light.r) * alpha, (setupLight.g - light.g) * alpha, (setupLight.b - light.b) * alpha, (setupLight.a - light.a) * alpha);
						dark.add((setupDark.r - dark.r) * alpha, (setupDark.g - dark.g) * alpha, (setupDark.b - dark.b) * alpha, 0);
				}
				return;
			}
			var r = 0, g = 0, b = 0, a = 0, r2 = 0, g2 = 0, b2 = 0;
			if (time >= frames[frames.length - TwoColorTimeline.ENTRIES]) {
				var i = frames.length;
				r = frames[i + TwoColorTimeline.PREV_R];
				g = frames[i + TwoColorTimeline.PREV_G];
				b = frames[i + TwoColorTimeline.PREV_B];
				a = frames[i + TwoColorTimeline.PREV_A];
				r2 = frames[i + TwoColorTimeline.PREV_R2];
				g2 = frames[i + TwoColorTimeline.PREV_G2];
				b2 = frames[i + TwoColorTimeline.PREV_B2];
			}
			else {
				var frame = Animation.binarySearch(frames, time, TwoColorTimeline.ENTRIES);
				r = frames[frame + TwoColorTimeline.PREV_R];
				g = frames[frame + TwoColorTimeline.PREV_G];
				b = frames[frame + TwoColorTimeline.PREV_B];
				a = frames[frame + TwoColorTimeline.PREV_A];
				r2 = frames[frame + TwoColorTimeline.PREV_R2];
				g2 = frames[frame + TwoColorTimeline.PREV_G2];
				b2 = frames[frame + TwoColorTimeline.PREV_B2];
				var frameTime = frames[frame];
				var percent = this.getCurvePercent(frame / TwoColorTimeline.ENTRIES - 1, 1 - (time - frameTime) / (frames[frame + TwoColorTimeline.PREV_TIME] - frameTime));
				r += (frames[frame + TwoColorTimeline.R] - r) * percent;
				g += (frames[frame + TwoColorTimeline.G] - g) * percent;
				b += (frames[frame + TwoColorTimeline.B] - b) * percent;
				a += (frames[frame + TwoColorTimeline.A] - a) * percent;
				r2 += (frames[frame + TwoColorTimeline.R2] - r2) * percent;
				g2 += (frames[frame + TwoColorTimeline.G2] - g2) * percent;
				b2 += (frames[frame + TwoColorTimeline.B2] - b2) * percent;
			}
			if (alpha == 1) {
				slot.color.set(r, g, b, a);
				slot.darkColor.set(r2, g2, b2, 1);
			}
			else {
				var light = slot.color, dark = slot.darkColor;
				if (pose == MixPose.setup) {
					light.setFromColor(slot.data.color);
					dark.setFromColor(slot.data.darkColor);
				}
				light.add((r - light.r) * alpha, (g - light.g) * alpha, (b - light.b) * alpha, (a - light.a) * alpha);
				dark.add((r2 - dark.r) * alpha, (g2 - dark.g) * alpha, (b2 - dark.b) * alpha, 0);
			}
		};
		TwoColorTimeline.ENTRIES = 8;
		TwoColorTimeline.PREV_TIME = -8;
		TwoColorTimeline.PREV_R = -7;
		TwoColorTimeline.PREV_G = -6;
		TwoColorTimeline.PREV_B = -5;
		TwoColorTimeline.PREV_A = -4;
		TwoColorTimeline.PREV_R2 = -3;
		TwoColorTimeline.PREV_G2 = -2;
		TwoColorTimeline.PREV_B2 = -1;
		TwoColorTimeline.R = 1;
		TwoColorTimeline.G = 2;
		TwoColorTimeline.B = 3;
		TwoColorTimeline.A = 4;
		TwoColorTimeline.R2 = 5;
		TwoColorTimeline.G2 = 6;
		TwoColorTimeline.B2 = 7;
		return TwoColorTimeline;
	}(CurveTimeline));
	spine.TwoColorTimeline = TwoColorTimeline;
	var AttachmentTimeline = (function () {
		function AttachmentTimeline(frameCount) {
			this.frames = spine.Utils.newFloatArray(frameCount);
			this.attachmentNames = new Array(frameCount);
		}
		AttachmentTimeline.prototype.getPropertyId = function () {
			return (TimelineType.attachment << 24) + this.slotIndex;
		};
		AttachmentTimeline.prototype.getFrameCount = function () {
			return this.frames.length;
		};
		AttachmentTimeline.prototype.setFrame = function (frameIndex, time, attachmentName) {
			this.frames[frameIndex] = time;
			this.attachmentNames[frameIndex] = attachmentName;
		};
		AttachmentTimeline.prototype.apply = function (skeleton, lastTime, time, events, alpha, pose, direction) {
			var slot = skeleton.slots[this.slotIndex];
			if (direction == MixDirection.out && pose == MixPose.setup) {
				var attachmentName_1 = slot.data.attachmentName;
				slot.setAttachment(attachmentName_1 == null ? null : skeleton.getAttachment(this.slotIndex, attachmentName_1));
				return;
			}
			var frames = this.frames;
			if (time < frames[0]) {
				if (pose == MixPose.setup) {
					var attachmentName_2 = slot.data.attachmentName;
					slot.setAttachment(attachmentName_2 == null ? null : skeleton.getAttachment(this.slotIndex, attachmentName_2));
				}
				return;
			}
			var frameIndex = 0;
			if (time >= frames[frames.length - 1])
				frameIndex = frames.length - 1;
			else
				frameIndex = Animation.binarySearch(frames, time, 1) - 1;
			var attachmentName = this.attachmentNames[frameIndex];
			skeleton.slots[this.slotIndex]
				.setAttachment(attachmentName == null ? null : skeleton.getAttachment(this.slotIndex, attachmentName));
		};
		return AttachmentTimeline;
	}());
	spine.AttachmentTimeline = AttachmentTimeline;
	var zeros = null;
	var DeformTimeline = (function (_super) {
		__extends(DeformTimeline, _super);
		function DeformTimeline(frameCount) {
			var _this = _super.call(this, frameCount) || this;
			_this.frames = spine.Utils.newFloatArray(frameCount);
			_this.frameVertices = new Array(frameCount);
			if (zeros == null)
				zeros = spine.Utils.newFloatArray(64);
			return _this;
		}
		DeformTimeline.prototype.getPropertyId = function () {
			return (TimelineType.deform << 27) + +this.attachment.id + this.slotIndex;
		};
		DeformTimeline.prototype.setFrame = function (frameIndex, time, vertices) {
			this.frames[frameIndex] = time;
			this.frameVertices[frameIndex] = vertices;
		};
		DeformTimeline.prototype.apply = function (skeleton, lastTime, time, firedEvents, alpha, pose, direction) {
			var slot = skeleton.slots[this.slotIndex];
			var slotAttachment = slot.getAttachment();
			if (!(slotAttachment instanceof spine.VertexAttachment) || !slotAttachment.applyDeform(this.attachment))
				return;
			var verticesArray = slot.attachmentVertices;
			if (verticesArray.length == 0)
				alpha = 1;
			var frameVertices = this.frameVertices;
			var vertexCount = frameVertices[0].length;
			var frames = this.frames;
			if (time < frames[0]) {
				var vertexAttachment = slotAttachment;
				switch (pose) {
					case MixPose.setup:
						verticesArray.length = 0;
						return;
					case MixPose.current:
						if (alpha == 1) {
							verticesArray.length = 0;
							break;
						}
						var vertices_1 = spine.Utils.setArraySize(verticesArray, vertexCount);
						if (vertexAttachment.bones == null) {
							var setupVertices = vertexAttachment.vertices;
							for (var i = 0; i < vertexCount; i++)
								vertices_1[i] += (setupVertices[i] - vertices_1[i]) * alpha;
						}
						else {
							alpha = 1 - alpha;
							for (var i = 0; i < vertexCount; i++)
								vertices_1[i] *= alpha;
						}
				}
				return;
			}
			var vertices = spine.Utils.setArraySize(verticesArray, vertexCount);
			if (time >= frames[frames.length - 1]) {
				var lastVertices = frameVertices[frames.length - 1];
				if (alpha == 1) {
					spine.Utils.arrayCopy(lastVertices, 0, vertices, 0, vertexCount);
				}
				else if (pose == MixPose.setup) {
					var vertexAttachment = slotAttachment;
					if (vertexAttachment.bones == null) {
						var setupVertices_1 = vertexAttachment.vertices;
						for (var i_1 = 0; i_1 < vertexCount; i_1++) {
							var setup = setupVertices_1[i_1];
							vertices[i_1] = setup + (lastVertices[i_1] - setup) * alpha;
						}
					}
					else {
						for (var i_2 = 0; i_2 < vertexCount; i_2++)
							vertices[i_2] = lastVertices[i_2] * alpha;
					}
				}
				else {
					for (var i_3 = 0; i_3 < vertexCount; i_3++)
						vertices[i_3] += (lastVertices[i_3] - vertices[i_3]) * alpha;
				}
				return;
			}
			var frame = Animation.binarySearch(frames, time);
			var prevVertices = frameVertices[frame - 1];
			var nextVertices = frameVertices[frame];
			var frameTime = frames[frame];
			var percent = this.getCurvePercent(frame - 1, 1 - (time - frameTime) / (frames[frame - 1] - frameTime));
			if (alpha == 1) {
				for (var i_4 = 0; i_4 < vertexCount; i_4++) {
					var prev = prevVertices[i_4];
					vertices[i_4] = prev + (nextVertices[i_4] - prev) * percent;
				}
			}
			else if (pose == MixPose.setup) {
				var vertexAttachment = slotAttachment;
				if (vertexAttachment.bones == null) {
					var setupVertices_2 = vertexAttachment.vertices;
					for (var i_5 = 0; i_5 < vertexCount; i_5++) {
						var prev = prevVertices[i_5], setup = setupVertices_2[i_5];
						vertices[i_5] = setup + (prev + (nextVertices[i_5] - prev) * percent - setup) * alpha;
					}
				}
				else {
					for (var i_6 = 0; i_6 < vertexCount; i_6++) {
						var prev = prevVertices[i_6];
						vertices[i_6] = (prev + (nextVertices[i_6] - prev) * percent) * alpha;
					}
				}
			}
			else {
				for (var i_7 = 0; i_7 < vertexCount; i_7++) {
					var prev = prevVertices[i_7];
					vertices[i_7] += (prev + (nextVertices[i_7] - prev) * percent - vertices[i_7]) * alpha;
				}
			}
		};
		return DeformTimeline;
	}(CurveTimeline));
	spine.DeformTimeline = DeformTimeline;
	var EventTimeline = (function () {
		function EventTimeline(frameCount) {
			this.frames = spine.Utils.newFloatArray(frameCount);
			this.events = new Array(frameCount);
		}
		EventTimeline.prototype.getPropertyId = function () {
			return TimelineType.event << 24;
		};
		EventTimeline.prototype.getFrameCount = function () {
			return this.frames.length;
		};
		EventTimeline.prototype.setFrame = function (frameIndex, event) {
			this.frames[frameIndex] = event.time;
			this.events[frameIndex] = event;
		};
		EventTimeline.prototype.apply = function (skeleton, lastTime, time, firedEvents, alpha, pose, direction) {
			if (firedEvents == null)
				return;
			var frames = this.frames;
			var frameCount = this.frames.length;
			if (lastTime > time) {
				this.apply(skeleton, lastTime, Number.MAX_VALUE, firedEvents, alpha, pose, direction);
				lastTime = -1;
			}
			else if (lastTime >= frames[frameCount - 1])
				return;
			if (time < frames[0])
				return;
			var frame = 0;
			if (lastTime < frames[0])
				frame = 0;
			else {
				frame = Animation.binarySearch(frames, lastTime);
				var frameTime = frames[frame];
				while (frame > 0) {
					if (frames[frame - 1] != frameTime)
						break;
					frame--;
				}
			}
			for (; frame < frameCount && time >= frames[frame]; frame++)
				firedEvents.push(this.events[frame]);
		};
		return EventTimeline;
	}());
	spine.EventTimeline = EventTimeline;
	var DrawOrderTimeline = (function () {
		function DrawOrderTimeline(frameCount) {
			this.frames = spine.Utils.newFloatArray(frameCount);
			this.drawOrders = new Array(frameCount);
		}
		DrawOrderTimeline.prototype.getPropertyId = function () {
			return TimelineType.drawOrder << 24;
		};
		DrawOrderTimeline.prototype.getFrameCount = function () {
			return this.frames.length;
		};
		DrawOrderTimeline.prototype.setFrame = function (frameIndex, time, drawOrder) {
			this.frames[frameIndex] = time;
			this.drawOrders[frameIndex] = drawOrder;
		};
		DrawOrderTimeline.prototype.apply = function (skeleton, lastTime, time, firedEvents, alpha, pose, direction) {
			var drawOrder = skeleton.drawOrder;
			var slots = skeleton.slots;
			if (direction == MixDirection.out && pose == MixPose.setup) {
				spine.Utils.arrayCopy(skeleton.slots, 0, skeleton.drawOrder, 0, skeleton.slots.length);
				return;
			}
			var frames = this.frames;
			if (time < frames[0]) {
				if (pose == MixPose.setup)
					spine.Utils.arrayCopy(skeleton.slots, 0, skeleton.drawOrder, 0, skeleton.slots.length);
				return;
			}
			var frame = 0;
			if (time >= frames[frames.length - 1])
				frame = frames.length - 1;
			else
				frame = Animation.binarySearch(frames, time) - 1;
			var drawOrderToSetupIndex = this.drawOrders[frame];
			if (drawOrderToSetupIndex == null)
				spine.Utils.arrayCopy(slots, 0, drawOrder, 0, slots.length);
			else {
				for (var i = 0, n = drawOrderToSetupIndex.length; i < n; i++)
					drawOrder[i] = slots[drawOrderToSetupIndex[i]];
			}
		};
		return DrawOrderTimeline;
	}());
	spine.DrawOrderTimeline = DrawOrderTimeline;
	var IkConstraintTimeline = (function (_super) {
		__extends(IkConstraintTimeline, _super);
		function IkConstraintTimeline(frameCount) {
			var _this = _super.call(this, frameCount) || this;
			_this.frames = spine.Utils.newFloatArray(frameCount * IkConstraintTimeline.ENTRIES);
			return _this;
		}
		IkConstraintTimeline.prototype.getPropertyId = function () {
			return (TimelineType.ikConstraint << 24) + this.ikConstraintIndex;
		};
		IkConstraintTimeline.prototype.setFrame = function (frameIndex, time, mix, bendDirection) {
			frameIndex *= IkConstraintTimeline.ENTRIES;
			this.frames[frameIndex] = time;
			this.frames[frameIndex + IkConstraintTimeline.MIX] = mix;
			this.frames[frameIndex + IkConstraintTimeline.BEND_DIRECTION] = bendDirection;
		};
		IkConstraintTimeline.prototype.apply = function (skeleton, lastTime, time, firedEvents, alpha, pose, direction) {
			var frames = this.frames;
			var constraint = skeleton.ikConstraints[this.ikConstraintIndex];
			if (time < frames[0]) {
				switch (pose) {