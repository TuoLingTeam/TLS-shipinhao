<script setup lang="ts">
defineProps<{
  name:
    | "dashboard"
    | "review"
    | "order"
    | "delivery"
    | "license"
    | "settings"
    | "spark"
    | "shield"
    | "storage";
  iconClass?: string;
}>();
</script>

<template>
  <svg
    v-if="name === 'dashboard'"
    viewBox="0 0 24 24"
    fill="none"
    stroke="currentColor"
    stroke-width="1.8"
    stroke-linecap="round"
    stroke-linejoin="round"
    :class="iconClass"
    aria-hidden="true"
  >
    <path d="M3 13.2h7.8V21H3z" />
    <path d="M13.2 3H21v8.2h-7.8z" />
    <path d="M13.2 13.2H21V21h-7.8z" />
    <path d="M3 3h7.8v6H3z" />
  </svg>

  <svg
    v-else-if="name === 'review'"
    viewBox="0 0 24 24"
    fill="none"
    stroke="currentColor"
    stroke-width="1.8"
    stroke-linecap="round"
    stroke-linejoin="round"
    :class="iconClass"
    aria-hidden="true"
  >
    <path d="m12 3 2.8 5.68 6.27.91-4.54 4.43 1.07 6.25L12 17.44 6.4 20.27l1.07-6.25L2.93 9.6l6.27-.91L12 3Z" />
  </svg>

  <svg
    v-else-if="name === 'order'"
    viewBox="0 0 24 24"
    fill="none"
    stroke="currentColor"
    stroke-width="1.8"
    stroke-linecap="round"
    stroke-linejoin="round"
    :class="iconClass"
    aria-hidden="true"
  >
    <path d="M3 7.5 12 3l9 4.5v9L12 21l-9-4.5z" />
    <path d="M12 12 3 7.5" />
    <path d="M12 12l9-4.5" />
    <path d="M12 21v-9" />
  </svg>

  <svg
    v-else-if="name === 'delivery'"
    viewBox="0 0 24 24"
    fill="none"
    stroke="currentColor"
    stroke-width="1.8"
    stroke-linecap="round"
    stroke-linejoin="round"
    :class="iconClass"
    aria-hidden="true"
  >
    <path d="M2.5 7.5h12v8h-12z" />
    <path d="M14.5 10h3l3 3v2.5h-6" />
    <circle cx="7.5" cy="18" r="1.5" />
    <circle cx="18" cy="18" r="1.5" />
  </svg>

  <svg
    v-else-if="name === 'license' || name === 'shield'"
    viewBox="0 0 24 24"
    fill="none"
    stroke="currentColor"
    stroke-width="1.8"
    stroke-linecap="round"
    stroke-linejoin="round"
    :class="iconClass"
    aria-hidden="true"
  >
    <path d="M12 3 19 6v5c0 4.4-2.83 8.45-7 9.75C7.83 19.45 5 15.4 5 11V6z" />
    <path d="m9.4 11.8 1.7 1.7 3.5-3.8" />
  </svg>

  <svg
    v-else-if="name === 'settings'"
    viewBox="0 0 24 24"
    fill="none"
    stroke="currentColor"
    stroke-width="1.8"
    stroke-linecap="round"
    stroke-linejoin="round"
    :class="iconClass"
    aria-hidden="true"
  >
    <path d="M12 8.5A3.5 3.5 0 1 1 8.5 12 3.5 3.5 0 0 1 12 8.5Z" />
    <path d="M19.4 15a1 1 0 0 0 .2 1.1l.03.03a1.9 1.9 0 0 1 0 2.68 1.9 1.9 0 0 1-2.69 0l-.03-.03a1 1 0 0 0-1.1-.2 1 1 0 0 0-.6.91V20a1.9 1.9 0 0 1-3.8 0v-.04a1 1 0 0 0-.6-.91 1 1 0 0 0-1.1.2l-.03.03a1.9 1.9 0 1 1-2.69-2.68l.03-.03a1 1 0 0 0 .2-1.1 1 1 0 0 0-.91-.6H4a1.9 1.9 0 0 1 0-3.8h.04a1 1 0 0 0 .91-.6 1 1 0 0 0-.2-1.1l-.03-.03a1.9 1.9 0 1 1 2.68-2.69l.03.03a1 1 0 0 0 1.1.2h.01a1 1 0 0 0 .6-.91V4a1.9 1.9 0 1 1 3.8 0v.04a1 1 0 0 0 .6.91 1 1 0 0 0 1.1-.2l.03-.03a1.9 1.9 0 1 1 2.69 2.68l-.03.03a1 1 0 0 0-.2 1.1v.01a1 1 0 0 0 .91.6H20a1.9 1.9 0 0 1 0 3.8h-.04a1 1 0 0 0-.91.6Z" />
  </svg>

  <svg
    v-else-if="name === 'spark'"
    viewBox="0 0 24 24"
    fill="none"
    stroke="currentColor"
    stroke-width="1.8"
    stroke-linecap="round"
    stroke-linejoin="round"
    :class="iconClass"
    aria-hidden="true"
  >
    <path d="m12 3 1.8 4.2L18 9l-4.2 1.8L12 15l-1.8-4.2L6 9l4.2-1.8L12 3Z" />
    <path d="M5 17.5 6 20l2.5 1-2.5 1L5 24l-1-2.5L1.5 20 4 19z" transform="translate(0 -2.5)" />
    <path d="m18.5 14 1 2.5 2.5 1-2.5 1-1 2.5-1-2.5-2.5-1 2.5-1z" />
  </svg>

  <svg
    v-else
    viewBox="0 0 24 24"
    fill="none"
    stroke="currentColor"
    stroke-width="1.8"
    stroke-linecap="round"
    stroke-linejoin="round"
    :class="iconClass"
    aria-hidden="true"
  >
    <ellipse cx="12" cy="6" rx="6.5" ry="3" />
    <path d="M5.5 6v6c0 1.66 2.91 3 6.5 3s6.5-1.34 6.5-3V6" />
    <path d="M5.5 12v6c0 1.66 2.91 3 6.5 3s6.5-1.34 6.5-3v-6" />
  </svg>
</template>
