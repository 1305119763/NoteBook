<script setup lang="ts">
import { computed } from "vue";
import { APP_DISPLAY_VERSION } from "../constants/appMeta";
import brandLogo from "../assets/notebook-logo.png";

const versionLabel = computed(() => `v${APP_DISPLAY_VERSION}`);
</script>

<template>
  <div class="nav-brand">
    <div class="nav-brand-visual">
      <img
        class="nav-brand-logo"
        :src="brandLogo"
        width="56"
        height="56"
        alt=""
        decoding="async"
      />
      <div class="nav-brand-text">
        <div class="nav-brand-title-row">
          <span class="nav-brand-title">NoteBook</span>
          <span class="nav-brand-version">{{ versionLabel }}</span>
        </div>
        <div class="nav-brand-sub">个人空间</div>
      </div>
    </div>
  </div>
</template>

<style scoped>
.nav-brand {
  padding: 0 6px 8px;
}

.nav-brand-visual {
  display: flex;
  align-items: center;
  gap: 12px;
}

.nav-brand-logo {
  width: 56px;
  height: 56px;
  object-fit: contain;
  flex-shrink: 0;
  border-radius: 50%;
  box-shadow: 0 2px 12px rgba(15, 23, 42, 0.1);
}

.nav-brand-text {
  min-width: 0;
  flex: 1;
}

.nav-brand-title-row {
  display: flex;
  flex-wrap: wrap;
  align-items: baseline;
  gap: 6px 8px;
}

.nav-brand-title {
  font-size: 20px;
  font-weight: 800;
  letter-spacing: -0.03em;
  color: var(--text);
  line-height: 1.2;
}

.nav-brand-version {
  font-size: 12px;
  font-weight: 600;
  color: #9ca3af;
  letter-spacing: 0.02em;
  line-height: 1.2;
  transform: translateY(1px);
}

.nav-brand-sub {
  margin-top: 4px;
  font-size: 10px;
  font-weight: 600;
  letter-spacing: 0.14em;
  text-transform: uppercase;
  color: var(--muted);
}
</style>

