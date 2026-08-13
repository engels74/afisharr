// SPDX-FileCopyrightText: 2026 Afisharr contributors
// SPDX-License-Identifier: AGPL-3.0-or-later

import type { PluralForms } from './plural';

/**
 * The English catalogue. Complete by construction: every key the interface
 * uses is declared here, and `MessageKey` is derived from this object, so a
 * key that is not in it is a type error rather than a missing translation at
 * runtime. There is no untranslated-key fallback because there is no way to
 * reach one (`I-UX-7`).
 *
 * Keys are dotted and grouped by where they appear. A key is never assembled
 * at a call site from a variable — `t(\`state.\${kind}\`)` would make the
 * catalogue unsearchable and the lint rule unable to see anything.
 */
export const en = {
	'app.name': 'Afisharr',
	'app.sourceLink': 'Source',
	'app.skipToContent': 'Skip to content',

	'api.unreachable':
		'Afisharr did not answer. It may be restarting, or the connection to it is down.',
	'api.unreadable':
		'Something answered for Afisharr, and this page could not read what it said. A proxy in front of the instance may be answering in its place.',

	'landing.unreachable':
		'Until this instance answers, Afisharr cannot tell whether it has been set up, so it will not guess where to send you.',

	'nav.dashboard': 'Dashboard',
	'nav.collections': 'Collections',
	'nav.design': 'Design',
	'nav.homeScreen': 'Home Screen',
	'nav.lifecycle': 'Lifecycle',
	'nav.doctor': 'Doctor',
	'nav.settings': 'Settings',
	'nav.primary': 'Primary',

	'appearance.mode.label': 'Color mode',
	'appearance.mode.system': 'Follow the system',
	'appearance.mode.light': 'Light',
	'appearance.mode.dark': 'Dark',

	'state.loading.label': 'Loading',
	'state.loading.stillWorking': 'Still working…',
	'state.empty.nothingCreated.title': 'Nothing here yet',
	'state.empty.nothingMatched.title': 'Nothing matched',
	'state.empty.nothingMatched.body':
		'The query succeeded and returned nothing. Narrowing predicate: {predicate}',
	'state.empty.pending.title': 'Not synced yet',
	'state.empty.pending.action': 'Run now',
	'state.error.title': 'That did not work',
	'state.error.retry': 'Retry',
	'state.error.details': 'Technical detail',
	'state.frozen.title': 'Held at last known good',
	'state.frozen.body': '{source} last succeeded {age} ago.',
	'state.degraded.title': 'Working, with something missing',
	'state.degraded.configure': 'Configure',
	'state.stale.title': 'Not refreshed',
	'state.stale.body':
		'These values are {age} old and are being preserved deliberately.',
	'state.pending.title': 'In flight',
	'state.pending.body': 'Committed, and not yet confirmed.',
	'state.blocked.title': 'Waiting on a decision',
	'state.blocked.retryAt': 'You can try again in {duration}.',
	'state.nonConvergent.title': 'Could not be settled',
	'state.nonConvergent.body':
		'The last verified order is shown. These will be retried.',

	'destructive.preview': 'Preview',
	'destructive.confirmTyped': 'Type {phrase} to confirm',
	'destructive.cancel': 'Cancel',
	'destructive.proceed': 'Proceed',
	'destructive.report': 'What happened',

	'stream.disconnected': 'Live updates are disconnected',
	'stream.reconnecting': 'Reconnecting…',

	'setup.claim.title': 'Claim this instance',
	'setup.claim.body':
		'Afisharr printed a setup token to the console when it started. Enter it to continue.',
	'setup.claim.tokenLabel': 'Setup token',
	'setup.claim.submit': 'Claim',
	'setup.claim.tokenExpired':
		'No token is live. Restart the container and read the console for a fresh one.',
	'setup.claim.recoveryTitle': 'Or sign in as the administrator',
	'setup.claim.recoveryBody':
		'This instance already has an administrator, so those credentials can claim the wizard.',
	'setup.claim.recoverySubmit': 'Claim with credentials',
	'setup.admin.title': 'Create the administrator',
	'setup.admin.body':
		'One account, and it holds every permission this instance has.',
	'setup.admin.submit': 'Create account',
	'setup.step': 'Step {ordinal} of 8',
	'setup.finish.pending': 'Finishing setup…',
	'setup.finish.consequence':
		'The administrator account exists, but setup is not finished, so signing in is still refused. Finishing it is the only step left.',

	'auth.username': 'Username',
	'auth.password': 'Password',
	'auth.signIn': 'Sign in',
	'auth.signOut': 'Sign out',
	'auth.signOutRefused':
		'Afisharr refused the sign-out, so this session is still live. Leaving this page now would leave it signed in for whoever opens this browser next.',
	'auth.title': 'Sign in',
	'auth.plexTitle': 'Or sign in with Plex',
	'auth.plexStart': 'Sign in with a code',
	'auth.plexOauthStart': 'Sign in at plex.tv',
	'auth.plexCode': 'Enter this code at plex.tv/link: {code}',
	'auth.plexWaiting': 'Waiting for Plex…',
	'auth.plexExpired': 'That Plex sign-in expired. Start it again.',
	'auth.sessionUnreachable':
		'Until this instance answers, Afisharr cannot tell who is signed in, so it will not show you a page that would only fail.',
	'auth.notAdministrator':
		'This account does not administer this instance, and Afisharr’s interface is administrator-only. Ask whoever runs it for administrator rights, or sign out and use another account.',

	'page.dashboard.title': 'Dashboard',
	'page.dashboard.empty':
		'Nothing has run yet. Once it has, this is where it appears.',
	'page.collections.title': 'Collections',
	'page.collections.empty':
		'A collection is a definition that builds and maintains a Plex collection.',
	'page.design.title': 'Design',
	'page.design.empty':
		'Poster and overlay templates, packs, and assets live here.',
	'page.homeScreen.title': 'Home Screen',
	'page.homeScreen.empty':
		'Ordering and visibility across the home surface and each library surface.',
	'page.lifecycle.title': 'Lifecycle',
	'page.lifecycle.empty':
		'Upcoming titles, placeholders, and acquisition activity.',
	'page.doctor.title': 'Doctor',
	'page.doctor.empty':
		'Everything that needs a decision, or is not right, appears here.',
	'page.settings.title': 'Settings',
	'page.settings.empty': 'Plex, integrations, libraries, users, and teardown.',
	'page.settings.plex': 'Plex',
	'page.settings.integrations': 'Integrations',
	'page.settings.libraries': 'Libraries',
	'page.settings.users': 'Users and API keys',
	'page.settings.general': 'General',
	'page.settings.teardown': 'Teardown',
	'page.settings.about': 'About',
	'page.notFound.title': 'No such page',
	'page.notFound.body':
		'That address does not resolve to anything on this instance.',
	'page.failed.title': 'This page could not be shown',
	'page.failed.body':
		'The address is right and something went wrong rendering it. Reloading may be enough; if it is not, the instance log has the detail.',
} as const satisfies Record<string, string>;

/**
 * Messages whose text depends on a count.
 *
 * Separate from the flat catalogue because their value is a set of forms
 * rather than a string, and folding both into one map would make every lookup
 * return a union the caller has to narrow.
 */
export const enPlurals = {
	'count.items': {
		one: '{count} item',
		other: '{count} items',
	},
	'count.collections': {
		one: '{count} collection',
		other: '{count} collections',
	},
	'count.sessionsRevoked': {
		one: '{count} other session was signed out',
		other: '{count} other sessions were signed out',
	},
	'count.minutes': {
		one: '{count} minute',
		other: '{count} minutes',
	},
	'count.seconds': {
		one: '{count} second',
		other: '{count} seconds',
	},
} as const satisfies Record<string, PluralForms>;
