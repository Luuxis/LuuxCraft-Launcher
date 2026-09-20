/**
 * 3D player preview built on skinview3d (three.js). Handles classic/slim
 * models, capes, zoom, rotation and panning. The character always walks: the
 * launcher offers no animation picker.
 */
import { useEffect, useRef } from "react";
import { SkinViewer, WalkingAnimation } from "skinview3d";

import { DEFAULT_SKIN_URL } from "../../lib/defaultSkin";

export type ModelName = "auto" | "default" | "slim";

const DEFAULT_POSE_Y = 0.45;

interface SkinViewer3DProps {
  skin: string | null;
  cape: string | null;
  model: ModelName;
  autoRotate: boolean;
  zoom: number;
  className?: string;
  /** Increment to recentre the camera. */
  resetToken?: number;
  /** Fired when the user grabs the model to move it. */
  onUserControl?: () => void;
}

export function SkinViewer3D({ skin, cape, model, autoRotate, zoom, className = "", resetToken = 0, onUserControl }: SkinViewer3DProps) {
  const container = useRef<HTMLDivElement>(null);
  const canvasRef = useRef<HTMLCanvasElement>(null);
  const viewer = useRef<SkinViewer | null>(null);
  // Kept in a ref so changing the callback never rebuilds the whole viewer.
  const userControl = useRef(onUserControl);
  userControl.current = onUserControl;

  // Create / dispose the viewer.
  useEffect(() => {
    const canvas = canvasRef.current;
    const box = container.current;
    if (!canvas || !box) return;
    const measure = () => {
      const rect = box.getBoundingClientRect();
      return { width: Math.max(1, Math.round(rect.width)), height: Math.max(1, Math.round(rect.height)) };
    };
    const initial = measure();
    const instance = new SkinViewer({
      canvas,
      width: initial.width,
      height: initial.height,
      fov: 50,
      zoom: 0.65,
      enableControls: true,
    });
    instance.controls.enablePan = true;
    instance.controls.enableZoom = true;
    instance.controls.enableRotate = true;
    instance.globalLight.intensity = 3;
    instance.cameraLight.intensity = 0.6;
    instance.autoRotateSpeed = 0.3;
    instance.playerWrapper.rotation.y = DEFAULT_POSE_Y;
    instance.animation = new WalkingAnimation();
    viewer.current = instance;

    // The canvas fills its container through CSS; the render buffer follows so
    // the picture is never stretched.
    const observer = new ResizeObserver(() => {
      const { width, height } = measure();
      instance.setSize(width, height);
    });
    observer.observe(box);

    // Taking hold of the model hands control back to the user: the automatic
    // rotation would otherwise keep fighting the drag.
    const onGrab = () => userControl.current?.();
    canvas.addEventListener("pointerdown", onGrab);

    return () => {
      canvas.removeEventListener("pointerdown", onGrab);
      observer.disconnect();
      instance.dispose();
      viewer.current = null;
    };
  }, []);

  // Skin texture + model.
  useEffect(() => {
    const instance = viewer.current;
    if (!instance) return;
    const options = { model: model === "auto" ? ("auto-detect" as const) : model };
    if (skin) {
      instance.loadSkin(skin, options).catch(() => {
        void instance.loadSkin(DEFAULT_SKIN_URL, { model: "default" });
      });
    } else {
      void instance.loadSkin(DEFAULT_SKIN_URL, { model: model === "auto" ? "default" : model });
    }
  }, [skin, model]);

  useEffect(() => {
    const instance = viewer.current;
    if (!instance) return;
    if (cape) {
      instance.loadCape(cape, { backEquipment: "cape" }).catch(() => instance.loadCape(null));
    } else {
      instance.loadCape(null);
    }
  }, [cape]);

  useEffect(() => {
    const instance = viewer.current;
    if (!instance) return;
    instance.autoRotate = autoRotate;
  }, [autoRotate]);

  useEffect(() => {
    const instance = viewer.current;
    if (!instance) return;
    instance.zoom = zoom;
  }, [zoom]);

  // "Recentrer" puts back both the camera and the default three-quarter pose.
  useEffect(() => {
    const instance = viewer.current;
    if (!instance || resetToken === 0) return;
    instance.resetCameraPose();
    instance.playerWrapper.rotation.y = DEFAULT_POSE_Y;
  }, [resetToken]);

  return (
    <div ref={container} className={`relative w-full h-full ${className}`}>
      <canvas ref={canvasRef} className="skin-canvas" style={{ touchAction: "none" }} />
    </div>
  );
}
