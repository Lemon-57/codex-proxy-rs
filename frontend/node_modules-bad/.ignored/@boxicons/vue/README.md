# @boxicons/vue

Vue 3 icon library built from Boxicons SVG files with full tree-shaking support.

## Installation

```bash
npm install @boxicons/vue
# or
yarn add @boxicons/vue
# or
pnpm add @boxicons/vue
```

## Usage

### Basic Usage

```vue
<script setup>
import { Alarm, Twitter, Home } from '@boxicons/vue';
</script>

<template>
  <Alarm />
  <Twitter />
  <Home />
</template>
```

### Icon Packs

- **basic** - Outline/regular style icons (default)
- **filled** - Solid/filled style icons
- **brands** - Brand/logo icons

```vue
<template>
  <Alarm />
  <Alarm pack="filled" />
</template>
```

### Sizing

#### Size Presets

```vue
<template>
  <Alarm size="xs" />   <!-- 16px -->
  <Alarm size="sm" />   <!-- 20px -->
  <Alarm size="base" /> <!-- 24px (default) -->
  <Alarm size="md" />   <!-- 36px -->
  <Alarm size="lg" />   <!-- 48px -->
  <Alarm size="xl" />   <!-- 64px -->
  <Alarm size="2xl" />  <!-- 96px -->
  <Alarm size="3xl" />  <!-- 128px -->
  <Alarm size="4xl" />  <!-- 256px -->
  <Alarm size="5xl" />  <!-- 512px -->
</template>
```

#### Custom Sizing

```vue
<template>
  <Alarm :width="32" :height="32" />
  <Alarm width="2rem" height="2rem" />
</template>
```

### Colors

```vue
<template>
  <Alarm fill="#ff0000" />
  <Alarm fill="currentColor" />
</template>
```

### Opacity

```vue
<template>
  <Alarm :opacity="0.5" />
</template>
```

### Flip

```vue
<template>
  <Alarm flip="horizontal" />
  <Alarm flip="vertical" />
</template>
```

### Rotate

```vue
<template>
  <Alarm :rotate="45" />
  <Alarm rotate="90deg" />
</template>
```

### Remove Padding

```vue
<template>
  <Alarm remove-padding />
</template>
```

### Combining Props

```vue
<template>
  <Alarm 
    pack="filled"
    fill="#ffffff"
    :opacity="0.8"
    size="lg"
    flip="horizontal"
    :rotate="45"
    class="my-icon"
  />
</template>
```

## Tree Shaking

Only imported icons are bundled:

```js
// ✅ Only Alarm is bundled
import { Alarm } from '@boxicons/vue';
```

## Available Icons

- **1884** basic (outline) icons
- **1884** filled icons  
- **295** brand icons

## TypeScript

```ts
import type { BoxIconProps, IconPack, IconSize, FlipDirection } from '@boxicons/vue';
```

## License

MIT
