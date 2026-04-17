export function isValidLicenseKey(key: string): boolean {
  return /^[A-Z0-9-]{8,32}$/i.test(key.trim());
}

export function isValidTrackingNumber(num: string): boolean {
  return /^[A-Z0-9]{6,30}$/i.test(num.trim());
}

export function isValidOrderId(id: string): boolean {
  return /^\d{10,25}$/.test(id.trim());
}
