declare module 'lucide-react' {
  import React from 'react';

  interface LucideProps {
    size?: number | string;
    color?: string;
    strokeWidth?: number | string;
    absoluteStrokeWidth?: boolean;
    className?: string;
  }

  export type LucideIcon = React.ForwardRefExoticComponent<
    Omit<React.SVGProps<SVGSVGElement>, 'ref'> & LucideProps & React.RefAttributes<SVGSVGElement>
  >;

  export const ArrowLeft: LucideIcon;
  export const Brain: LucideIcon;
  export const Check: LucideIcon;
  export const ChevronDown: LucideIcon;
  export const ChevronRight: LucideIcon;
  export const Copy: LucideIcon;
  export const CornerUpLeft: LucideIcon;
  export const ExternalLink: LucideIcon;
  export const Eye: LucideIcon;
  export const EyeOff: LucideIcon;
  export const Send: LucideIcon;
  export const Settings: LucideIcon;
  export const Square: LucideIcon;
  export const X: LucideIcon;
}
