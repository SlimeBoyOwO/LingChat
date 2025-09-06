<template>
    <div class="effect-star-field">
        <canvas ref="canvas" />
    </div>
</template>
<script setup lang="ts">
import { ref, watch } from "vue";

import ThrowHelper from "../../api/services/ThrowHelper";
import { SingleAudio } from "../../api/services/UIStatus";
import { settings } from "../../api/store";
import { IEffect, IEffectConfig } from "../../api/types/Effect";

type Star = {
    v: Vector3;
    color: string;
};

interface StarFieldConfig extends IEffectConfig {
    starCount: number;
    starSize: number;
    attractorSize: number;
    scrollSpeed: number;
    directionChangeRate: number;
    colors: string[];
}

type Vector2 = {
    x: number;
    y: number;
};

type Vector3 = Vector2 & {
    z: number;
};

const canvas = ref<HTMLCanvasElement | null>(null);
var dir = Math.PI;
const { config } = defineProps<{ config: StarFieldConfig }>();
const stars: Star[] = [];
watch(
    () => canvas.value,
    () => {
        if (canvas.value === null) return;
        const ctx = canvas.value.getContext("2d")!;
        for (let i = 0; i < config.starCount; i++) {
            const z = Math.random();
            const color = config.colors[Math.floor(Math.random() * config.colors.length)];
            stars.push({
                v: {
                    x: randomInt(canvas.value!.width * 1000),
                    y: randomInt(canvas.value!.height * 1000),
                    z: z + 0.5
                },
                color
            });
        }
        const update = (timestamp: number) => {
            dir = Math.sin((timestamp / 13289) * config.directionChangeRate) * Math.PI;
            ctx.clearRect(0, 0, canvas.value!.width, canvas.value!.height);
            ctx.globalCompositeOperation = "lighter";

            const dx = Math.cos(dir) * config.scrollSpeed;
            const dy = Math.sin(dir) * config.scrollSpeed;

            stars.forEach(star => {
                star.v.x += star.v.x * dx;
                star.v.y += star.v.y * dy;

                const x =
                    mod(star.v.x, canvas.value!.width + config.starSize + config.attractorSize) -
                    (config.starSize / 2 + config.attractorSize / 2);
                const y =
                    mod(star.v.y, canvas.value!.height + config.starSize + config.attractorSize) -
                    (config.starSize / 2 + config.attractorSize / 2);

                ctx!.fillStyle = star.color;
                ctx!.fillRect(x, y, config.starSize * star.v.z, config.starSize * star.v.z);
            });

            ctx!.globalCompositeOperation = "source-over";
            animationId = requestAnimationFrame(update);
        };
        update();
    }
);

// Helper methods
function randomInt(max: number, min: number = 0): number {
    if (max < min) {
        console.warn("Max should not be less than min");
    }
    return Math.floor(Math.random() * (max - min) + min);
}

function mod(value: number, mod: number): number {
    return ((value % mod) + mod) % mod;
}
</script>
<style lang=""></style>
