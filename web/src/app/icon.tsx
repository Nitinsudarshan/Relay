import { ImageResponse } from 'next/og'

// Route segment config
export const runtime = 'edge'

// Image metadata
export const alt = 'Relay — AI Voice & Memory'
export const size = {
  width: 32,
  height: 32,
}
export const contentType = 'image/png'

// Image generation
export default function Icon() {
  return new ImageResponse(
    (
      <div
        style={{
          background: 'transparent',
          width: '100%',
          height: '100%',
          display: 'flex',
          alignItems: 'center',
          justifyContent: 'center',
          padding: '2px',
        }}
      >
        <svg
          width="28"
          height="28"
          viewBox="0 0 24 24"
          fill="none"
          xmlns="http://www.w3.org/2000/svg"
        >
          <rect x="3" y="3" width="3.2" height="18" rx="1.6" fill="#171717" />
          <rect x="8.5" y="3" width="8.5" height="3.2" rx="1.6" fill="#2563EB" />
          <rect x="8.5" y="10.4" width="12.5" height="3.2" rx="1.6" fill="#2563EB" />
          <rect x="8.5" y="17.8" width="8.5" height="3.2" rx="1.6" fill="#2563EB" />
        </svg>
      </div>
    ),
    {
      ...size,
    }
  )
}
