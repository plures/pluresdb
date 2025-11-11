// App-wide state stores using Svelte 5 runes

import { writable } from "svelte/store";

// Theme management
export const theme = writable<"light" | "dark">("light");

// Sidebar state
export const sidebarOpen = writable(true);

// Loading state
export const loading = writable(false);

// Mobile menu state
export const mobileMenuOpen = writable(false);

// Notification state
export const notifications = writable<
  Array<{
    id: string;
    type: "success" | "error" | "warning" | "info";
    title: string;
    message: string;
    duration?: number;
  }>
>([]);

// User preferences
export const preferences = writable({
  autoSave: true,
  theme: "light",
  sidebarCollapsed: false,
  notifications: true,
  animations: true,
});

// Add notification helper
export function addNotification(notification: {
  type: "success" | "error" | "warning" | "info";
  title: string;
  message: string;
  duration?: number;
}) {
  const id = Math.random().toString(36).substr(2, 9);
  const newNotification = { id, ...notification };

  notifications.update((notifications) => [...notifications, newNotification]);

  // Auto-remove after duration
  const duration = notification.duration || 5000;
  setTimeout(() => {
    removeNotification(id);
  }, duration);
}

// Remove notification helper
export function removeNotification(id: string) {
  notifications.update((notifications) =>
    notifications.filter((notification) => notification.id !== id)
  );
}

// Clear all notifications
export function clearNotifications() {
  notifications.set([]);
}
