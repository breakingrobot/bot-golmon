// Vérification à la compilation : les réponses décrites par la spécification OpenAPI du bot
// (types générés par Hey API) doivent convenir aux types écrits à la main (types.ts) que
// les pages utilisent. `npm run check:api` régénère src/api/generated puis compile ce fichier
// (tsconfig.contract.json) ; il est exclu du `typecheck` ordinaire, le dossier généré n'étant pas versionné.

import type * as G from './generated/types.gen'
import type * as T from './types'
import type * as S from './types-site'

/** Erreur de compilation si `Server` ne convient pas à `Client`. */
type Fits<Server extends Client, Client> = [Server, Client]

export type ContractChecks = [
  Fits<G.GetMeResponse, T.Me>,
  Fits<G.GetGuildStatsResponse, T.GuildStats>,
  Fits<G.SearchMembersResponse, T.MemberSearch>,
  Fits<G.GetMemberProfileResponse, T.MemberProfile>,
  Fits<G.GetTransactionsResponse, T.TransactionsResponse>,
  Fits<G.GetCotisationsResponse, T.CotisationsResponse>,
  Fits<G.CreateCotisationResponse, T.CotisationEntry>,
  Fits<G.GetCotisationObjectiveResponse, T.Objective | null>,
  Fits<G.GetCoffreMetaResponse, T.CoffreMeta>,
  Fits<G.GetCoffreStockResponse, T.CoffreStock>,
  Fits<G.GetCoffreMovementsResponse, T.Page<T.CoffreMovement>>,
  Fits<G.GetCoffrePlayersResponse, T.CoffrePlayer[]>,
  Fits<G.GetCoffreSeriesResponse, T.CoffreSeries>,
  Fits<G.GetCoffreAlertsResponse, T.CoffreAlerts>,
  Fits<G.SetCoffreInventoryResponse, T.CoffreInventoryResult>,
  Fits<T.CoffreInventory, G.CoffreInventory>,
  Fits<G.GetContactsResponse, T.ContactsResponse>,
  Fits<G.GetContactResponse, T.ContactDetail>,
  Fits<G.CreateContactResponse, T.Contact>,
  Fits<G.CreateContactNoteResponse, T.ContactNote>,
  Fits<G.GetJournalResponse, T.JournalResponse>,
  Fits<G.GetPermissionsResponse, T.PermissionsResponse>,
  Fits<G.GetConfigStatusResponse, T.ConfigStatus>,
  Fits<G.GetTicketsResponse, T.TicketsResponse>,
  Fits<G.GetTicketStaffResponse, T.TicketStats>,
  Fits<G.GetTicketLogResponse, T.TicketLog>,
  // Accueil et santé.
  Fits<G.GetGuildOverviewResponse, S.GuildOverview>,
  Fits<G.GetGuildHealthResponse, S.GuildHealth>,
  // Présences, absences, recrues, annuaire, écritures des transactions.
  Fits<G.GetPresencesResponse, T.Page<T.PresenceSummary>>,
  Fits<G.GetPresenceResponse, T.PresenceDetail>,
  Fits<G.GetPresenceStatsResponse, T.PresenceStats>,
  Fits<G.CreatePresenceResponse, T.PresenceSummary>,
  Fits<G.ClosePresenceResponse, T.PresenceDetail>,
  Fits<G.GetPresenceSchedulesResponse, T.PresenceSchedule[]>,
  Fits<G.CreatePresenceScheduleResponse, T.PresenceSchedule>,
  Fits<T.NewPresence, G.NewPresenceInput>,
  Fits<T.NewPresenceSchedule, G.NewScheduleInput>,
  Fits<G.GetAbsencesResponse, T.AbsencesResponse>,
  Fits<G.CreateAbsenceResponse, T.DeclaredAbsence>,
  Fits<G.EndAbsenceResponse, T.EndedAbsence>,
  Fits<T.NewAbsence, G.NewAbsenceInput>,
  Fits<G.GetRecruesResponse, T.RecruesResponse>,
  Fits<G.GetRecrueHistoryResponse, T.RecrueHistoryWeek[]>,
  Fits<G.VoteRecrueResponse, T.Recrue['myVote'] & object>,
  Fits<G.SetRecrueVetoResponse, T.Veto>,
  Fits<T.RecrueVoteInput, G.RecrueVoteInput>,
  Fits<G.GetVehiclesResponse, T.VehiclesResponse>,
  Fits<G.CreateVehicleResponse, T.VehicleEntry>,
  Fits<T.VehicleInput, G.VehicleInput>,
  Fits<G.GetPhonesResponse, T.PhoneEntry[]>,
  Fits<G.SetPhoneResponse, T.PhoneEntry>,
  Fits<G.CreateTransactionResponse, T.Transaction>,
  Fits<G.SetTransactionStatusResponse, T.Transaction>,
  Fits<T.NewTransaction, G.NewTransactionInput>,
  Fits<T.TransactionItemInput, G.TransactionItemInput>,
]
