import { useId, type CSSProperties } from 'react';
import './treeGlyph.css';

type Stop = readonly [offset: number, token: string, opacity?: number];

/** Gradient stops read their colors from the `--logo-*` custom properties, so the mark follows the theme. */
function Stops({ stops }: { stops: readonly Stop[] }) {
  return (
    <>
      {stops.map(([offset, token, opacity]) => (
        <stop
          key={offset}
          offset={`${offset}%`}
          style={{ stopColor: `var(--logo-${token})` }}
          stopOpacity={opacity}
        />
      ))}
    </>
  );
}

const fill = (token: string): CSSProperties => ({ fill: `var(--logo-${token})` });
const stroke = (token: string): CSSProperties => ({ stroke: `var(--logo-${token})` });

const SECTORS = [
  'M 120 38 A 84 84 0 0 1 182 65',
  'M 186 70 A 84 84 0 0 1 203 134',
  'M 201 140 A 84 84 0 0 1 168 191',
  'M 162 194 A 84 84 0 0 1 92 201',
  'M 86 199 A 84 84 0 0 1 41 150',
  'M 39 144 A 84 84 0 0 1 54 74',
  'M 59 69 A 84 84 0 0 1 114 38',
];

const PCB_TRACES = [
  'M 111.5 214 L 111.5 196 L 108.5 192 L 108.5 182',
  'M 116.5 214 L 116.5 201',
  'M 123.5 214 L 123.5 188 L 126.5 184 L 126.5 174',
  'M 130 214 L 130 193',
];

/** The app logo, shown in the sidebar header (`frameless`) and in the empty state. Decorative, themed by CSS tokens. */
export function TreeGlyph({ size = 26, frameless = false }: { size?: number; frameless?: boolean }) {
  const uid = `bf${useId().replace(/[^a-zA-Z0-9]/g, '')}`;
  const ref = (name: string) => `url(#${uid}-${name})`;
  const id = (name: string) => `${uid}-${name}`;

  const treeMark = (
    <>
      <path
        d="M 106 164 L 134 164 L 136 212 C 136 214.2 134.2 216 132 216 L 108 216 C 105.8 216 104 214.2 104 212 Z"
        fill={ref('trunk')}
        style={stroke('trunk-edge')}
        strokeOpacity="0.35"
        strokeWidth="0.9"
      />
      {!frameless && (
        <>
          <g style={stroke('trunk-line')} strokeOpacity="0.32" strokeWidth="1.35" strokeLinecap="round" strokeLinejoin="round">
            {PCB_TRACES.map((d) => (
              <path key={d} d={d} />
            ))}
          </g>
          <g strokeWidth="1.6" strokeLinecap="round" strokeLinejoin="round" fill="none">
            <path className="bf-pcb-1" d={PCB_TRACES[0]} />
            <path className="bf-pcb-2" d={PCB_TRACES[2]} />
            <path className="bf-pcb-1" d={PCB_TRACES[3]} />
          </g>
          <circle className="bf-node" cx="108.5" cy="181" r="1.8" />
          <circle className="bf-node bf-node-2" cx="116.5" cy="199.5" r="1.5" />
          <circle className="bf-node" cx="126.5" cy="173" r="1.8" />
          <circle className="bf-node bf-node-2" cx="130" cy="191.5" r="1.6" />
        </>
      )}
      <g className="bf-tier bf-t6" filter={frameless ? undefined : ref('shadow')}>
        <path
          d="M 116 130 C 145 142 174 159 195 178 C 197 180 195.5 182.5 192.5 182.5 C 166 183 139 175 113 160 Z"
          fill={ref('deep')}
          style={stroke('edge-a')}
          strokeOpacity="0.22"
          strokeWidth="0.75"
        />
      </g>
      <g className="bf-tier bf-t5" filter={frameless ? undefined : ref('shadow')}>
        <path
          d="M 121 118 C 96 135 68 153 45 173 C 43 175 44.5 177.8 47.5 177.8 C 78 178.5 108 167 132 146 Z"
          fill={ref('light')}
          style={stroke('edge-b')}
          strokeOpacity="0.28"
          strokeWidth="0.75"
        />
      </g>
      <g className="bf-tier bf-t4" filter={frameless ? undefined : ref('shadow')}>
        <path
          d="M 115 104 C 99 121 84 138 73 155 C 71.5 157.2 73.2 159.2 75.8 159 C 98 157.5 118 147 133 132 Z"
          fill={ref('mid')}
          style={stroke('edge-c')}
          strokeOpacity="0.2"
          strokeWidth="0.7"
        />
      </g>
      <g className="bf-tier bf-t3" filter={frameless ? undefined : ref('shadow')}>
        <path
          d="M 112 76 C 94 92 75 108 60 125 C 58.2 127 59.6 129.6 62.4 129.6 C 85 129.5 107 119 124 101 Z"
          fill={ref('light')}
          style={stroke('edge-b')}
          strokeOpacity="0.3"
          strokeWidth="0.75"
        />
      </g>
      <g className="bf-tier bf-t2" filter={frameless ? undefined : ref('shadow')}>
        <path
          d="M 116 74 C 138 91 160 109 178 130 C 179.8 132 178.2 134.6 175.4 134.5 C 151 133.5 129 120 112 97 Z"
          fill={ref('mid')}
          style={stroke('edge-c')}
          strokeOpacity="0.28"
          strokeWidth="0.75"
        />
      </g>
      <g className="bf-tier bf-t1" filter={frameless ? undefined : ref('shadow')}>
        <path
          d="M 120 31 C 121.2 31 122.4 32 123.5 33.5 C 138 51 153 67 166 81.5 C 167.6 83.3 166.3 85.8 163.8 85.8 C 146 86 131 80 120 70.5 C 109 80 94 86 76.2 85.8 C 73.7 85.8 72.4 83.3 74 81.5 C 87 67 102 51 116.5 33.5 C 117.6 32 118.8 31 120 31 Z"
          fill={ref('top')}
          style={stroke('edge-d')}
          strokeOpacity="0.48"
          strokeWidth="0.85"
        />
        {!frameless && <circle className="bf-apex" cx="120" cy="34" r="3.2" filter={ref('glow')} />}
      </g>
    </>
  );

  return (
    <svg
      className={frameless ? 'brand-logo bf-logo bf-logo-frameless' : 'brand-logo bf-logo'}
      width={size}
      height={size}
      viewBox={frameless ? '38 24 164 196' : '0 0 240 240'}
      fill="none"
      xmlns="http://www.w3.org/2000/svg"
      aria-hidden="true"
      focusable="false"
    >
      <defs>
        <linearGradient id={id('rim')} x1="24" y1="12" x2="216" y2="228" gradientUnits="userSpaceOnUse">
          <Stops stops={[[0, 'rim-1', 0.72], [35, 'rim-2', 0.45], [70, 'rim-3', 0.85], [100, 'rim-4', 0.55]]} />
        </linearGradient>
        <radialGradient id={id('bg')} cx="50%" cy="34%" r="72%">
          <Stops stops={[[0, 'bg-1'], [52, 'bg-2'], [100, 'bg-3']]} />
        </radialGradient>
        <radialGradient id={id('platter')} cx="50%" cy="50%" r="50%">
          <Stops stops={[[0, 'platter-1', 0.16], [58, 'platter-2', 0.07], [100, 'platter-3', 0]]} />
        </radialGradient>
        <linearGradient id={id('beam')} x1="120" y1="122" x2="178" y2="54" gradientUnits="userSpaceOnUse">
          <Stops stops={[[0, 'ink', 0.34], [65, 'ink', 0.12], [100, 'ink', 0]]} />
        </linearGradient>
        <linearGradient id={id('top')} x1="120" y1="30" x2="120" y2="86" gradientUnits="userSpaceOnUse">
          <Stops stops={[[0, 'top-1'], [48, 'top-2'], [100, 'top-3']]} />
        </linearGradient>
        <linearGradient id={id('light')} x1="65" y1="80" x2="145" y2="165" gradientUnits="userSpaceOnUse">
          <Stops stops={[[0, 'light-1'], [50, 'light-2'], [100, 'light-3']]} />
        </linearGradient>
        <linearGradient id={id('mid')} x1="175" y1="86" x2="92" y2="176" gradientUnits="userSpaceOnUse">
          <Stops stops={[[0, 'mid-1'], [52, 'mid-2'], [100, 'mid-3']]} />
        </linearGradient>
        <linearGradient id={id('deep')} x1="48" y1="130" x2="188" y2="184" gradientUnits="userSpaceOnUse">
          <Stops stops={[[0, 'deep-1'], [55, 'deep-2'], [100, 'deep-3']]} />
        </linearGradient>
        <linearGradient id={id('trunk')} x1="120" y1="166" x2="120" y2="216" gradientUnits="userSpaceOnUse">
          <Stops stops={[[0, 'trunk-1'], [45, 'trunk-2'], [100, 'trunk-3']]} />
        </linearGradient>
        <clipPath id={id('clip')}>
          <rect x="10" y="10" width="220" height="220" rx="48" />
        </clipPath>
        <filter id={id('shadow')} x="-20%" y="-20%" width="140%" height="145%">
          <feDropShadow dx="0" dy="4.5" stdDeviation="4" floodOpacity="0.62" style={{ floodColor: 'var(--logo-shadow)' }} />
        </filter>
        <filter id={id('glow')} x="-45%" y="-45%" width="190%" height="190%">
          <feGaussianBlur stdDeviation="3.2" result="blur" />
          <feMerge>
            <feMergeNode in="blur" />
            <feMergeNode in="SourceGraphic" />
          </feMerge>
        </filter>
      </defs>

      {frameless ? (
        treeMark
      ) : (
        <>
          <rect x="5" y="5" width="230" height="230" rx="52" style={fill('tile')} stroke={ref('rim')} strokeWidth="2" />
          <g clipPath={ref('clip')}>
            <rect x="10" y="10" width="220" height="220" fill={ref('bg')} />
            <circle cx="120" cy="122" r="95" fill={ref('platter')} />
            <circle cx="120" cy="122" r="92" style={stroke('ring')} strokeOpacity="0.14" strokeWidth="1" />
            <circle cx="120" cy="122" r="76" style={stroke('ring')} strokeOpacity="0.11" strokeWidth="0.8" />
            <circle cx="120" cy="122" r="60" style={stroke('ring')} strokeOpacity="0.09" strokeWidth="0.8" />
            <circle cx="120" cy="122" r="43" style={stroke('ring')} strokeOpacity="0.08" strokeWidth="0.8" />
            <g className="bf-platter-cw">
              <circle cx="120" cy="122" r="76" style={stroke('ring-2')} strokeOpacity="0.22" strokeWidth="1.2" strokeDasharray="3 9 14 9" />
              <circle cx="120" cy="122" r="43" style={stroke('ring-2')} strokeOpacity="0.18" strokeWidth="1" strokeDasharray="2 6" />
            </g>
            <g className="bf-platter-ccw">
              <circle cx="120" cy="122" r="60" style={stroke('ring')} strokeOpacity="0.2" strokeWidth="1.1" strokeDasharray="8 12 2 12" />
            </g>
            <g fill="none" strokeLinecap="butt">
              {SECTORS.map((d, i) => (
                <path key={d} className={`bf-sector bf-s${i + 1}`} d={d} />
              ))}
            </g>
            <g className="bf-scanner-arm">
              <path d="M 120 122 L 120 30 A 92 92 0 0 1 178 51 Z" fill={ref('beam')} />
              <line x1="120" y1="122" x2="120" y2="30" style={stroke('ink')} strokeWidth="1.4" strokeOpacity="0.72" />
              <circle cx="120" cy="30" r="2.4" style={fill('ink')} />
            </g>
            {treeMark}
            <g filter={ref('glow')}>
              <line className="bf-pt-line bf-d1" x1="63" y1="88" x2="63" y2="103" strokeWidth="1.2" strokeLinecap="round" strokeOpacity="0.45" />
              <circle className="bf-pt bf-d1" cx="63" cy="87" r="2.2" />
              <line className="bf-pt-line bf-d2" x1="54" y1="104" x2="54" y2="115" strokeWidth="1" strokeLinecap="round" strokeOpacity="0.35" />
              <circle className="bf-pt bf-d2" cx="54" cy="103" r="1.6" />
              <circle className="bf-pt bf-d3" cx="55" cy="138" r="2.1" />
              <rect className="bf-pt bf-d4" x="45" y="147" width="2.8" height="2.8" rx="0.7" />
              <line className="bf-pt-line bf-d5" x1="173" y1="96" x2="173" y2="110" strokeWidth="1.1" strokeLinecap="round" strokeOpacity="0.4" />
              <circle className="bf-pt bf-d5" cx="173" cy="95" r="2.1" />
              <circle className="bf-pt bf-d2" cx="182" cy="106" r="1.5" />
              <line className="bf-pt-line bf-d6" x1="186" y1="136" x2="186" y2="152" strokeWidth="1.2" strokeLinecap="round" strokeOpacity="0.45" />
              <circle className="bf-pt bf-d6" cx="186" cy="153" r="2.2" />
              <rect className="bf-pt bf-d3" x="193" y="143" width="2.6" height="2.6" rx="0.7" />
            </g>
            <rect x="10.5" y="10.5" width="219" height="219" rx="47.5" style={stroke('inner-rim')} strokeOpacity="0.1" strokeWidth="1" />
          </g>
        </>
      )}
    </svg>
  );
}
