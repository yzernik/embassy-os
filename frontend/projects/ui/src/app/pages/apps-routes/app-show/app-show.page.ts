import { ChangeDetectionStrategy, Component } from '@angular/core'
import { NavController } from '@ionic/angular'
import { PatchDbService } from 'src/app/services/patch-db/patch-db.service'
import { PackageDataEntry } from 'src/app/services/patch-db/data-model'
import { PackageState } from 'src/app/types/package-state'
import {
  PackageStatus,
  PrimaryStatus,
} from 'src/app/services/pkg-status-rendering.service'
import {
  ConnectionFailure,
  ConnectionService,
} from 'src/app/services/connection.service'
import { map, startWith } from 'rxjs/operators'
import { ActivatedRoute } from '@angular/router'

const STATES = [
  PackageState.Installing,
  PackageState.Updating,
  PackageState.Restoring,
]

@Component({
  selector: 'app-show',
  templateUrl: './app-show.page.html',
  changeDetection: ChangeDetectionStrategy.OnPush,
})
export class AppShowPage {
  private readonly pkgId = this.route.snapshot.paramMap.get('pkgId')

  readonly pkg$ = this.patch.watch$('package-data', this.pkgId).pipe(
    map(pkg => {
      // if package disappears, navigate to list page
      if (!pkg) {
        this.navCtrl.navigateRoot('/services')
      }

      return { ...pkg }
    }),
    startWith(this.patch.getData()['package-data'][this.pkgId]),
  )

  readonly connectionFailure$ = this.connectionService
    .watchFailure$()
    .pipe(map(failure => failure !== ConnectionFailure.None))

  constructor(
    private readonly route: ActivatedRoute,
    private readonly navCtrl: NavController,
    private readonly patch: PatchDbService,
    private readonly connectionService: ConnectionService,
  ) {}

  isInstalled(
    { state }: PackageDataEntry,
    { primary }: PackageStatus,
  ): boolean {
    return (
      state === PackageState.Installed && primary !== PrimaryStatus.BackingUp
    )
  }

  isRunning({ primary }: PackageStatus): boolean {
    return primary === PrimaryStatus.Running
  }

  showProgress({ state }: PackageDataEntry): boolean {
    return STATES.includes(state)
  }
}
