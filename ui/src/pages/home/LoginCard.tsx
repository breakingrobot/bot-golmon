import { LogIn } from 'lucide-react'

import type { Me } from '@/api/types'
import { Button } from '@/atoms/Button'
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from '@/atoms/Card'
import { LoginLink } from '@/atoms/LoginLink'

/** Retour de `/auth/callback` ou `/login` en erreur (`/?connexion=refusee|indisponible`). */
export function loginProblem(): string | null {
  switch (new URLSearchParams(window.location.search).get('connexion')) {
    case 'refusee':
      return 'La connexion a échoué ou a expiré. Réessayez.'
    case 'indisponible':
      return "La connexion Discord n'est pas configurée sur ce bot (DISCORD_CLIENT_SECRET et WEB_URL)."
    default:
      return null
  }
}

export function LoginCard({ me }: { me: Me }) {
  const problem = loginProblem()
  return (
    <div className="mx-auto mt-8 max-w-md">
      {problem && (
        <p
          role="alert"
          className="mb-4 rounded-md border border-danger/40 bg-danger-soft px-4 py-3 text-sm text-danger"
        >
          {problem}
        </p>
      )}
      <Card>
        <CardHeader>
          <CardTitle className="text-xl">Connexion</CardTitle>
          <CardDescription>
            {me.loginEnabled
              ? 'Connectez-vous avec Discord pour consulter les tickets et les pages de vos serveurs.'
              : "La connexion Discord n'est pas configurée : seuls les logs de tickets sont accessibles, par leur lien."}
          </CardDescription>
        </CardHeader>
        {me.loginEnabled && (
          <CardContent className="space-y-3">
            <Button asChild className="w-full" size="lg">
              <LoginLink loginUrl={me.loginUrl}>
                <LogIn aria-hidden /> Se connecter avec Discord
              </LoginLink>
            </Button>
            <p className="text-xs text-ink-subtle">
              Discord ne partage que votre pseudo et votre avatar : le site ne lit ni vos serveurs ni vos messages.
            </p>
          </CardContent>
        )}
      </Card>
    </div>
  )
}
