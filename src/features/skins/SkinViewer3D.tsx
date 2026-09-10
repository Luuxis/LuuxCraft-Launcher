/**
 * 3D player preview built on skinview3d (three.js). Handles classic/slim
 * models, capes (cape or elytra), animations, zoom, rotation and panning.
 */
import { useEffect, useRef } from "react";
import {
  FlyingAnimation,
  IdleAnimation,
  RunningAnimation,
  SkinViewer,
  WalkingAnimation,
  WaveAnimation,
  type PlayerAnimation,
} from "skinview3d";

import { defaultSkinCanvas } from "../../lib/defaultSkin";

export type AnimationName = "none" | "idle" | "walk" | "run" | "wave" | "fly";
export type ModelName = "auto" | "default" | "slim";

interface SkinViewer3DProps {
  skin: string | null;
  cape: string | null;
  model: ModelName;
  animation: AnimationName;
  autoRotate: boolean;
  zoom: number;
  elytra: boolean;
  nameTag?: string | null;
  className?: string;
  /** Increment to recentre the camera. */
  resetToken?: number;
}

function makeAnimation(name: AnimationName): PlayerAnimation | null {
  switch (name) {
    case "idle":
      return new IdleAnimation();
    case "walk":
      return new WalkingAnimation();
    case "run":
      return new RunningAnimation();
    case "wave":
      return new WaveAnimation();
    case "fly":
      return new FlyingAnimation();
    default:
      return null;
  }
}

export function SkinViewer3D({ skin, cape, model, animation, autoRotate, zoom, elytra, nameTag, className = "", resetToken = 0 }: SkinViewer3DProps) {
  const container = useRef<HTMLDivElement>(null);
  const canvasRef = useRef<HTMLCanvasElement>(null);
  const viewer = useRef<SkinViewer | null>(null);

  // Create / dispose the viewer.
  useEffect(() => {
    const canvas = canvasRef.current;
    const box = container.current;
    if (!canvas || !box) return;
    const instance = new SkinViewer({
      canvas,
      width: box.clientWidth || 320,
      height: box.clientHeight || 420,
      fov: 50,
      zoom: 0.9,
      enableControls: true,
    });
    instance.controls.enablePan = true;
    instance.controls.enableZoom = true;
    instance.controls.enableRotate = true;
    instance.globalLight.intensity = 3;
    instance.cameraLight.intensity = 0.6;
    instance.autoRotateSpeed = 0.6;
    viewer.current = instance;

    const observer = new ResizeObserver(() => {
      if (box.clientWidth > 0 && box.clientHeight > 0) instance.setSize(box.clientWidth, box.clientHeight);
    });
    observer.observe(box);
    return () => {
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
        instance.loadSkin(defaultSkinCanvas(), { model: "default" });
      });
    } else {
      instance.loadSkin(defaultSkinCanvas(), { model: model === "auto" ? "default" : model });
    }
  }, [skin, model]);

  // Cape / elytra.
  useEffect(() => {
    const instance = viewer.current;
    if (!instance) return;
    if (cape) {
      instance.loadCape(cape, { backEquipment: elytra ? "elytra" : "cape" }).catch(() => instance.loadCape(null));
    } else {
      instance.loadCape(null);
    }
  }, [cape, elytra]);

  useEffect(() => {
    const instance = viewer.current;
    if (!instance) return;
    instance.animation = makeAnimation(animation);
  }, [animation]);

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

  useEffect(() => {
    const instance = viewer.current;
    if (!instance) return;
    instance.nameTag = nameTag ?? null;
  }, [nameTag]);

  useEffect(() => {
    const instance = viewer.current;
    if (!instance || resetToken === 0) return;
    instance.resetCameraPose();
  }, [resetToken]);

  return (
    <div ref={container} className={`relative w-full h-full ${className}`}>
      <canvas ref={canvasRef} className="block w-full h-full" style={{ touchAction: "none" }} />
    </div>
  );
}
