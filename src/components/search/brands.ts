import {
  siApple,
  siSamsung,
  siOneplus,
  siNikon,
  siSony,
  siHuawei,
  siXiaomi,
  siLeica,
  siGoogle,
  siLg,
  siMotorola,
  siKodak,
  siFujifilm,
  siPanasonic,
  siInstagram,
  siEricsson,
  siOppo,
  siVivo,
  siAsus,
  siBlackberry,
} from 'simple-icons';

export const BRANDS: Record<string, { path: string; hex: string }> = {
  apple: siApple,
  samsung: siSamsung,
  oneplus: siOneplus,
  nikon: siNikon,
  sony: siSony,
  huawei: siHuawei,
  xiaomi: siXiaomi,
  leica: siLeica,
  google: siGoogle,
  lg: siLg,
  motorola: siMotorola,
  kodak: siKodak,
  fujifilm: siFujifilm,
  panasonic: siPanasonic,
  instagram: siInstagram,
  ericsson: siEricsson,
  oppo: siOppo,
  vivo: siVivo,
  asus: siAsus,
  blackberry: siBlackberry,
};

export function brandMeta(name: string): { path: string; hex: string } | null {
  const b = name.toLowerCase().replace(/[^a-z0-9]/g, '');
  for (const [key, icon] of Object.entries(BRANDS)) {
    if (b.includes(key)) return icon;
  }
  return null;
}
