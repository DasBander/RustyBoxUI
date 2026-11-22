export const games = [
  {
    id: 'afterlight',
    title: 'Afterlight',
    wordmark: 'Afterlight',
    genre: 'Open world / Adventure',
    size: 24.8,
    new: true,
    description:
      'Cross the rooftops of a ruined city and reach the last watchtower before sundown.',
    credit: 'Image from Sintel, © Blender Foundation.',
  },
  {
    id: 'orbit',
    title: 'Orbit: Far From Home',
    wordmark: 'Orbit',
    subtitle: 'Far From Home',
    genre: 'Sci-fi / Exploration',
    size: 18.2,
    new: true,
    description:
      'Pilot a patched-up freighter through an asteroid field with a crew that barely fits the cockpit.',
    credit: 'Image from Wing It!, © Blender Foundation.',
  },
  {
    id: 'everwild',
    title: 'The Wild Between',
    wordmark: 'The Wild\nBetween',
    genre: 'Adventure / Exploration',
    size: 12.6,
    description:
      'Lead a lost flock across the valley and use your staff to wake the creatures under the forest floor.',
    credit: 'Image from Spring, © Blender Foundation.',
  },
  {
    id: 'drift',
    title: 'Nightshift',
    wordmark: 'Nightshift',
    genre: 'Racing / Arcade',
    size: 32.4,
    description:
      'Take a rear-wheel-drive coupe onto a closed circuit. Link the corners and beat your last drift score.',
    credit: 'Photograph by Dmitry Tomashek.',
  },
  {
    id: 'hollow',
    title: 'Hollow Kingdom',
    wordmark: 'Hollow\nKingdom',
    genre: 'Action / RPG',
    size: 21.3,
    description: 'Guide four stranded campers through a forest where the trails change after dark.',
    credit: 'Image from Sprite Fright, © Blender Foundation.',
  },
  {
    id: 'coast',
    title: 'Tidebound',
    wordmark: 'Tidebound',
    genre: 'Cozy / Adventure',
    size: 6.8,
    description:
      'Repair a lighthouse on the Atlantic coast. Take the skiff between coves to collect supplies before high tide.',
    credit: 'Photograph by Graham Powell-Wood.',
  },
  {
    id: 'signal',
    title: 'The Last Signal',
    wordmark: 'The Last\nSignal',
    genre: 'Mystery / Survival',
    size: 16.1,
    description:
      'Trace a radio transmission through the service tunnels of a power station on its last backup battery.',
    credit: 'Image from Charge, © Blender Foundation.',
  },
].map((g) => ({ ...g, art: `art/${g.id}.webp`, cover: `art/${g.id}-cover.webp` }));
